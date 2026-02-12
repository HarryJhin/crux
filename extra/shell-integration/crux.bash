# Crux terminal emulator — bash shell integration
#
# Emits OSC 7 (CWD reporting) and OSC 133 (FinalTerm prompt marking)
# escape sequences so Crux can track the working directory and
# distinguish prompt, input, and output regions.
#
# Source this file in ~/.bashrc or let Crux auto-source it:
#   [[ "$TERM_PROGRAM" == "Crux" ]] && source /path/to/crux.bash

# Guard: only run inside Crux.
[[ "$TERM_PROGRAM" == "Crux" ]] || return 0

# --- OSC 7: Report CWD after each command -----------------------------------

__crux_osc7() {
    printf '\e]7;file://%s%s\a' "${HOSTNAME}" "${PWD}"
}

# --- OSC 133: FinalTerm prompt marking ---------------------------------------

# Track whether we have already sent at least one D marker.
__crux_first_prompt=1

__crux_prompt_command() {
    local last_exit=$?

    # D — Command complete (skip on the very first prompt).
    if [[ -z "$__crux_first_prompt" ]]; then
        printf '\e]133;D;%d\a' "$last_exit"
    fi
    __crux_first_prompt=

    # A — Prompt start.
    printf '\e]133;A\a'

    # Report CWD.
    __crux_osc7
}

# B+C — Command start and output start (user pressed Enter).
# Uses the DEBUG trap which fires before each command.
__crux_preexec() {
    # The DEBUG trap also fires for PROMPT_COMMAND itself and subshells.
    # Only emit B+C when a real user command is about to run.
    if [[ "$BASH_COMMAND" == "__crux_prompt_command" ]]; then
        return
    fi
    # Guard against re-entry within a single command line.
    if [[ -n "$__crux_in_command" ]]; then
        return
    fi
    __crux_in_command=1
    printf '\e]133;B\a'
    printf '\e]133;C\a'
}

__crux_reset_command_flag() {
    __crux_in_command=
}

# --- Hook registration ------------------------------------------------------

# Prepend our prompt command, preserving any existing PROMPT_COMMAND.
if [[ -z "$PROMPT_COMMAND" ]]; then
    PROMPT_COMMAND="__crux_prompt_command; __crux_reset_command_flag"
elif [[ "$PROMPT_COMMAND" != *"__crux_prompt_command"* ]]; then
    PROMPT_COMMAND="__crux_prompt_command; __crux_reset_command_flag; ${PROMPT_COMMAND}"
fi

# Install the DEBUG trap for preexec emulation.
trap '__crux_preexec' DEBUG
