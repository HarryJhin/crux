# Crux terminal emulator — fish shell integration
#
# Emits OSC 7 (CWD reporting) and OSC 133 (FinalTerm prompt marking)
# escape sequences so Crux can track the working directory and
# distinguish prompt, input, and output regions.
#
# Source this file in ~/.config/fish/config.fish or let Crux auto-source it:
#   if test "$TERM_PROGRAM" = "Crux"
#       source /path/to/crux.fish
#   end

# Guard: only run inside Crux.
if test "$TERM_PROGRAM" != "Crux"
    exit 0
end

# Track whether we have sent at least one D marker.
set -g __crux_first_prompt 1

# --- fish_prompt: A + OSC 7 (prompt start + CWD) ----------------------------

function __crux_fish_prompt --on-event fish_prompt
    set -l last_status $status

    # D — Command complete (skip on the very first prompt).
    if test -z "$__crux_first_prompt"
        printf '\e]133;D;%d\a' $last_status
    end
    set -g __crux_first_prompt ""

    # A — Prompt start.
    printf '\e]133;A\a'

    # OSC 7 — Report CWD.
    printf '\e]7;file://%s%s\a' (hostname) $PWD
end

# --- fish_preexec: B + C (command start + output start) ----------------------

function __crux_fish_preexec --on-event fish_preexec
    printf '\e]133;B\a'
    printf '\e]133;C\a'
end
