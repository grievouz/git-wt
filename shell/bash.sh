
# git-wt integration v0.1.0
git() {
    if [ "$1" = "wt" ]; then
        shift
        
        local output
        local cd_path=""
        
        # Capture only stdout, let stderr pass through (for interactive prompts)
        while IFS= read -r line; do
            if [[ "$line" =~ ^CD:(.+)$ ]]; then
                # Extract path after "CD:"
                cd_path="${BASH_REMATCH[1]}"
            else
                # Print normal output
                echo "$line"
            fi
        done < <(command git-wt "$@")
        
        local exit_code=$?
        
        # If we got a CD directive, change directory
        if [ -n "$cd_path" ] && [ -d "$cd_path" ]; then
            cd "$cd_path" || return 1
        fi
        
        return $exit_code
    else
        command git "$@"
    fi
}
