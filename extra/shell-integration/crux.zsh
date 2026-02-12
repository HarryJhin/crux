# Crux terminal emulator — zsh shell integration
#
# Emits OSC 7 (CWD reporting) and OSC 133 (FinalTerm prompt marking)
# escape sequences so Crux can track the working directory and
# distinguish prompt, input, and output regions.
#
# Source this file in ~/.zshrc or let Crux auto-source it:
#   [[ "$TERM_PROGRAM" == "Crux" ]] && source /path/to/crux.zsh

# Guard: only run inside Crux.
[[ "$TERM_PROGRAM" == "Crux" ]] || return 0

# --- OSC 7: Report CWD after each command -----------------------------------

__crux_osc7() {
    emulate -L zsh
    printf '\e]7;file://%s%s\a' "${HOST}" "${PWD}"
}

# --- OSC 133: FinalTerm prompt marking ---------------------------------------

# A — Prompt start (before the prompt is drawn)
__crux_prompt_start() {
    emulate -L zsh
    printf '\e]133;A\a'
}

# D — Command complete (after the previous command finishes, before prompt)
# Must run before prompt_start so D comes before A in the byte stream.
__crux_command_complete() {
    emulate -L zsh
    printf '\e]133;D;%d\a' "$__crux_last_exit"
}

# Capture exit code early in precmd, before other hooks can clobber $?.
__crux_precmd() {
    __crux_last_exit=$?
}

# B — Command start (user pressed Enter; before the command executes)
# C — Output start (immediately after B; command output follows)
__crux_preexec() {
    emulate -L zsh
    printf '\e]133;B\a'
    printf '\e]133;C\a'
}

# --- Hook registration ------------------------------------------------------

# Track whether we have already sent at least one D marker.
# The very first prompt has no preceding command, so skip D.
__crux_first_prompt=1

__crux_precmd_wrapper() {
    __crux_precmd
    if [[ -z "$__crux_first_prompt" ]]; then
        __crux_command_complete
    fi
    __crux_first_prompt=
    __crux_prompt_start
    __crux_osc7
}

# Install hooks, avoiding duplicates.
if (( ${precmd_functions[(I)__crux_precmd_wrapper]} == 0 )); then
    precmd_functions=(__crux_precmd_wrapper $precmd_functions)
fi
if (( ${preexec_functions[(I)__crux_preexec]} == 0 )); then
    preexec_functions=(__crux_preexec $preexec_functions)
fi
