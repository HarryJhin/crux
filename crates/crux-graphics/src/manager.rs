//! Image storage and lifecycle management.
//!
//! The [`ImageManager`] is the central store for all images transmitted via
//! graphics protocols. It enforces a configurable memory quota (default 320MB)
//! and evicts least-recently-used images when the quota is exceeded.

use std::collections::HashMap;

use crate::error::GraphicsError;
use crate::types::{ImageData, ImageId, ImagePlacement};

/// Default memory quota: 320 MiB.
const DEFAULT_QUOTA_BYTES: usize = 320 * 1024 * 1024;

/// Maximum single image size: 64 MiB.
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;

/// Maximum number of pending chunked transfers to prevent resource exhaustion.
const MAX_PENDING_CHUNKS: usize = 32;

/// Maximum accumulated size for a single chunked transfer: 64 MiB.
const MAX_CHUNK_ACCUMULATION: usize = 64 * 1024 * 1024;

/// Internal record for a stored image.
#[derive(Debug)]
struct StoredImage {
    data: ImageData,
    /// Monotonically increasing access counter for LRU eviction.
    last_access: u64,
}

/// Manages image storage, placement tracking, and memory quota enforcement.
///
/// Images are stored in RAM keyed by [`ImageId`]. When the total memory
/// usage exceeds the quota, the least-recently-used images are evicted
/// until usage drops below the quota.
#[derive(Debug)]
pub struct ImageManager {
    /// Stored images keyed by their ID.
    images: HashMap<u32, StoredImage>,
    /// Image placements keyed by image ID, then placement ID.
    placements: HashMap<u32, Vec<ImagePlacement>>,
    /// Pending chunked transfers: image_id -> accumulated base64 data.
    pending_chunks: HashMap<u32, Vec<u8>>,
    /// Current total memory usage in bytes.
    total_bytes: usize,
    /// Maximum memory quota in bytes.
    quota_bytes: usize,
    /// Monotonic counter for LRU tracking.
    access_counter: u64,
    /// Next auto-assigned image ID.
    next_auto_id: u32,
}

impl Default for ImageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageManager {
    /// Create a new image manager with the default quota (320 MiB).
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            placements: HashMap::new(),
            pending_chunks: HashMap::new(),
            total_bytes: 0,
            quota_bytes: DEFAULT_QUOTA_BYTES,
            access_counter: 0,
            next_auto_id: 1,
        }
    }

    /// Create a new image manager with a custom quota.
    pub fn with_quota(quota_bytes: usize) -> Self {
        Self {
            quota_bytes,
            ..Self::new()
        }
    }

    /// Returns the current total memory usage in bytes.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns the configured memory quota in bytes.
    pub fn quota_bytes(&self) -> usize {
        self.quota_bytes
    }

    /// Returns the number of stored images.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Allocate the next auto-assigned image ID.
    pub fn next_image_id(&mut self) -> ImageId {
        let id = self.next_auto_id;
        self.next_auto_id = self.next_auto_id.wrapping_add(1);
        if self.next_auto_id == 0 {
            self.next_auto_id = 1; // Skip 0
        }
        ImageId(id)
    }

    /// Store an image. If an image with the same ID already exists, it is replaced.
    ///
    /// Enforces the per-image size limit and triggers LRU eviction if the
    /// total quota would be exceeded.
    pub fn store_image(&mut self, id: ImageId, data: ImageData) -> Result<(), GraphicsError> {
        let size = data.byte_size();

        // Check per-image limit
        if size > MAX_IMAGE_BYTES {
            return Err(GraphicsError::ImageTooLarge {
                size,
                max: MAX_IMAGE_BYTES,
            });
        }

        // Remove old image if it exists (reclaim its memory)
        if let Some(old) = self.images.remove(&id.0) {
            self.total_bytes = self.total_bytes.saturating_sub(old.data.byte_size());
        }

        // Evict LRU images until we have room
        while self.total_bytes + size > self.quota_bytes && !self.images.is_empty() {
            self.evict_lru();
        }

        // Final quota check after eviction
        if self.total_bytes + size > self.quota_bytes {
            return Err(GraphicsError::QuotaExceeded {
                used: self.total_bytes + size,
                quota: self.quota_bytes,
            });
        }

        self.access_counter += 1;
        self.images.insert(
            id.0,
            StoredImage {
                data,
                last_access: self.access_counter,
            },
        );
        self.total_bytes += size;

        Ok(())
    }

    /// Retrieve image data by ID, updating the LRU counter.
    pub fn get_image(&mut self, id: ImageId) -> Result<&ImageData, GraphicsError> {
        self.access_counter += 1;
        let counter = self.access_counter;
        let stored = self
            .images
            .get_mut(&id.0)
            .ok_or(GraphicsError::ImageNotFound(id))?;
        stored.last_access = counter;
        Ok(&stored.data)
    }

    /// Check if an image exists without updating the LRU counter.
    pub fn has_image(&self, id: ImageId) -> bool {
        self.images.contains_key(&id.0)
    }

    /// Delete an image and all its placements.
    pub fn delete_image(&mut self, id: ImageId) -> Result<(), GraphicsError> {
        if let Some(stored) = self.images.remove(&id.0) {
            self.total_bytes = self.total_bytes.saturating_sub(stored.data.byte_size());
            self.placements.remove(&id.0);
            Ok(())
        } else {
            Err(GraphicsError::ImageNotFound(id))
        }
    }

    /// Delete all images and placements.
    pub fn delete_all(&mut self) {
        self.images.clear();
        self.placements.clear();
        self.pending_chunks.clear();
        self.total_bytes = 0;
    }

    /// Add a placement for an existing image.
    pub fn place_image(&mut self, placement: ImagePlacement) -> Result<(), GraphicsError> {
        let id = placement.image_id;
        if !self.images.contains_key(&id.0) {
            return Err(GraphicsError::ImageNotFound(id));
        }
        self.placements
            .entry(id.0)
            .or_default()
            .push(placement);
        Ok(())
    }

    /// Delete a specific placement.
    pub fn delete_placement(
        &mut self,
        image_id: ImageId,
        placement_id: u32,
    ) -> Result<(), GraphicsError> {
        let placements = self
            .placements
            .get_mut(&image_id.0)
            .ok_or(GraphicsError::PlacementNotFound {
                image_id,
                placement_id,
            })?;

        let initial_len = placements.len();
        placements.retain(|p| p.placement_id != placement_id);

        if placements.len() == initial_len {
            return Err(GraphicsError::PlacementNotFound {
                image_id,
                placement_id,
            });
        }

        // Clean up empty placement vectors
        if placements.is_empty() {
            self.placements.remove(&image_id.0);
        }

        Ok(())
    }

    /// Get all placements for images that intersect the given row range.
    ///
    /// Returns placements where the placement row falls within `[start_row, end_row)`.
    pub fn get_placements_in_range(&mut self, start_row: i32, end_row: i32) -> Vec<&ImagePlacement> {
        let mut result = Vec::new();
        for placements in self.placements.values() {
            for placement in placements {
                if placement.row >= start_row && placement.row < end_row {
                    result.push(placement);
                }
            }
        }
        // Sort by z-index for correct layering
        result.sort_by_key(|p| p.z_index);
        result
    }

    /// Append a chunk of data for a chunked transfer.
    ///
    /// Returns an error if the pending chunks limit is exceeded or the
    /// accumulated data for this transfer exceeds the maximum chunk size.
    pub fn append_chunk(&mut self, image_id: u32, data: &[u8]) -> Result<(), GraphicsError> {
        // Check if we're at the pending chunks limit
        if !self.pending_chunks.contains_key(&image_id)
            && self.pending_chunks.len() >= MAX_PENDING_CHUNKS
        {
            // Drop the oldest pending transfer (first key in iteration order)
            if let Some(&oldest_id) = self.pending_chunks.keys().next() {
                log::warn!(
                    "dropping oldest pending chunked transfer (id={}) due to pending chunks limit",
                    oldest_id
                );
                self.pending_chunks.remove(&oldest_id);
            }
        }

        let accumulated = self.pending_chunks.entry(image_id).or_default();

        // Check accumulated size before adding new chunk
        let new_size = accumulated.len() + data.len();
        if new_size > MAX_CHUNK_ACCUMULATION {
            let size = new_size; // Capture before removing
            self.pending_chunks.remove(&image_id);
            return Err(GraphicsError::ImageTooLarge {
                size,
                max: MAX_CHUNK_ACCUMULATION,
            });
        }

        accumulated.extend_from_slice(data);
        Ok(())
    }

    /// Complete a chunked transfer, returning the accumulated data.
    pub fn complete_chunked_transfer(&mut self, image_id: u32) -> Option<Vec<u8>> {
        self.pending_chunks.remove(&image_id)
    }

    /// Check if there is a pending chunked transfer for the given image ID.
    pub fn has_pending_chunks(&self, image_id: u32) -> bool {
        self.pending_chunks.contains_key(&image_id)
    }

    /// Evict the least-recently-used image.
    fn evict_lru(&mut self) {
        if let Some((&lru_id, _)) = self
            .images
            .iter()
            .min_by_key(|(_, stored)| stored.last_access)
        {
            if let Some(stored) = self.images.remove(&lru_id) {
                self.total_bytes = self.total_bytes.saturating_sub(stored.data.byte_size());
                self.placements.remove(&lru_id);
                log::debug!(
                    "evicted image {} ({} bytes), total now {} bytes",
                    lru_id,
                    stored.data.byte_size(),
                    self.total_bytes
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PixelFormat;

    fn make_image(size: usize) -> ImageData {
        ImageData::new(vec![0u8; size], 1, 1, PixelFormat::Bgra)
    }

    #[test]
    fn test_store_and_retrieve() {
        let mut mgr = ImageManager::new();
        let id = ImageId(1);
        let img = make_image(100);
        mgr.store_image(id, img).unwrap();

        assert!(mgr.has_image(id));
        assert_eq!(mgr.image_count(), 1);
        assert_eq!(mgr.total_bytes(), 100);

        let data = mgr.get_image(id).unwrap();
        assert_eq!(data.byte_size(), 100);
    }

    #[test]
    fn test_delete_image() {
        let mut mgr = ImageManager::new();
        let id = ImageId(1);
        mgr.store_image(id, make_image(100)).unwrap();
        mgr.delete_image(id).unwrap();

        assert!(!mgr.has_image(id));
        assert_eq!(mgr.image_count(), 0);
        assert_eq!(mgr.total_bytes(), 0);
    }

    #[test]
    fn test_delete_nonexistent_image() {
        let mut mgr = ImageManager::new();
        let result = mgr.delete_image(ImageId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_all() {
        let mut mgr = ImageManager::new();
        mgr.store_image(ImageId(1), make_image(100)).unwrap();
        mgr.store_image(ImageId(2), make_image(200)).unwrap();
        mgr.delete_all();

        assert_eq!(mgr.image_count(), 0);
        assert_eq!(mgr.total_bytes(), 0);
    }

    #[test]
    fn test_replace_existing_image() {
        let mut mgr = ImageManager::new();
        let id = ImageId(1);
        mgr.store_image(id, make_image(100)).unwrap();
        assert_eq!(mgr.total_bytes(), 100);

        mgr.store_image(id, make_image(200)).unwrap();
        assert_eq!(mgr.image_count(), 1);
        assert_eq!(mgr.total_bytes(), 200);
    }

    #[test]
    fn test_quota_enforcement() {
        let mut mgr = ImageManager::with_quota(500);
        mgr.store_image(ImageId(1), make_image(200)).unwrap();
        mgr.store_image(ImageId(2), make_image(200)).unwrap();

        // This should trigger eviction of image 1
        mgr.store_image(ImageId(3), make_image(200)).unwrap();

        assert!(!mgr.has_image(ImageId(1))); // evicted (LRU)
        assert!(mgr.has_image(ImageId(2)));
        assert!(mgr.has_image(ImageId(3)));
        assert!(mgr.total_bytes() <= 500);
    }

    #[test]
    fn test_lru_eviction_order() {
        let mut mgr = ImageManager::with_quota(300);
        mgr.store_image(ImageId(1), make_image(100)).unwrap();
        mgr.store_image(ImageId(2), make_image(100)).unwrap();
        mgr.store_image(ImageId(3), make_image(100)).unwrap();

        // Access image 1 to make it recently used
        let _ = mgr.get_image(ImageId(1));

        // Adding image 4 should evict image 2 (LRU), not image 1
        mgr.store_image(ImageId(4), make_image(100)).unwrap();

        assert!(mgr.has_image(ImageId(1))); // recently accessed
        assert!(!mgr.has_image(ImageId(2))); // evicted (LRU)
        assert!(mgr.has_image(ImageId(3)));
        assert!(mgr.has_image(ImageId(4)));
    }

    #[test]
    fn test_per_image_size_limit() {
        let mut mgr = ImageManager::new();
        let result = mgr.store_image(ImageId(1), make_image(MAX_IMAGE_BYTES + 1));
        assert!(matches!(result, Err(GraphicsError::ImageTooLarge { .. })));
    }

    #[test]
    fn test_place_image() {
        let mut mgr = ImageManager::new();
        let id = ImageId(1);
        mgr.store_image(id, make_image(100)).unwrap();

        let placement = ImagePlacement::new(id);
        mgr.place_image(placement).unwrap();

        let placements = mgr.get_placements_in_range(0, 100);
        assert_eq!(placements.len(), 1);
    }

    #[test]
    fn test_place_nonexistent_image() {
        let mut mgr = ImageManager::new();
        let placement = ImagePlacement::new(ImageId(999));
        let result = mgr.place_image(placement);
        assert!(matches!(result, Err(GraphicsError::ImageNotFound(_))));
    }

    #[test]
    fn test_delete_placement() {
        let mut mgr = ImageManager::new();
        let id = ImageId(1);
        mgr.store_image(id, make_image(100)).unwrap();

        let mut placement = ImagePlacement::new(id);
        placement.placement_id = 5;
        mgr.place_image(placement).unwrap();

        mgr.delete_placement(id, 5).unwrap();
        let placements = mgr.get_placements_in_range(0, 100);
        assert!(placements.is_empty());
    }

    #[test]
    fn test_placements_sorted_by_z_index() {
        let mut mgr = ImageManager::new();
        let id = ImageId(1);
        mgr.store_image(id, make_image(100)).unwrap();

        let mut p1 = ImagePlacement::new(id);
        p1.placement_id = 1;
        p1.z_index = 10;
        mgr.place_image(p1).unwrap();

        let mut p2 = ImagePlacement::new(id);
        p2.placement_id = 2;
        p2.z_index = -5;
        mgr.place_image(p2).unwrap();

        let mut p3 = ImagePlacement::new(id);
        p3.placement_id = 3;
        p3.z_index = 0;
        mgr.place_image(p3).unwrap();

        let placements = mgr.get_placements_in_range(0, 100);
        assert_eq!(placements.len(), 3);
        assert_eq!(placements[0].z_index, -5);
        assert_eq!(placements[1].z_index, 0);
        assert_eq!(placements[2].z_index, 10);
    }

    #[test]
    fn test_chunked_transfer() {
        let mut mgr = ImageManager::new();
        mgr.append_chunk(1, b"AAAA").unwrap();
        assert!(mgr.has_pending_chunks(1));
        mgr.append_chunk(1, b"BBBB").unwrap();

        let data = mgr.complete_chunked_transfer(1).unwrap();
        assert_eq!(data, b"AAAABBBB");
        assert!(!mgr.has_pending_chunks(1));
    }

    #[test]
    fn test_auto_id_assignment() {
        let mut mgr = ImageManager::new();
        let id1 = mgr.next_image_id();
        let id2 = mgr.next_image_id();
        assert_eq!(id1, ImageId(1));
        assert_eq!(id2, ImageId(2));
    }

    #[test]
    fn test_placement_range_filtering() {
        let mut mgr = ImageManager::new();
        let id = ImageId(1);
        mgr.store_image(id, make_image(100)).unwrap();

        let mut p1 = ImagePlacement::new(id);
        p1.placement_id = 1;
        p1.row = 5;
        mgr.place_image(p1).unwrap();

        let mut p2 = ImagePlacement::new(id);
        p2.placement_id = 2;
        p2.row = 15;
        mgr.place_image(p2).unwrap();

        // Only p1 should be in range [0, 10)
        let placements = mgr.get_placements_in_range(0, 10);
        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].row, 5);

        // Both should be in range [0, 20)
        let placements = mgr.get_placements_in_range(0, 20);
        assert_eq!(placements.len(), 2);
    }

    #[test]
    fn test_eviction_removes_placements() {
        let mut mgr = ImageManager::with_quota(200);
        let id1 = ImageId(1);
        mgr.store_image(id1, make_image(100)).unwrap();
        mgr.place_image(ImagePlacement::new(id1)).unwrap();

        let id2 = ImageId(2);
        mgr.store_image(id2, make_image(100)).unwrap();

        // This should evict id1 along with its placements
        let id3 = ImageId(3);
        mgr.store_image(id3, make_image(100)).unwrap();

        assert!(!mgr.has_image(id1));
        let placements = mgr.get_placements_in_range(0, 100);
        assert!(placements.iter().all(|p| p.image_id != id1));
    }
}
