
# git-wt: standalone wt alias
wt() {
    local cd_path=""

    while IFS= read -r line; do
        if [[ "$line" =~ ^CD:(.+)$ ]]; then
            cd_path="${BASH_REMATCH[1]}"
        else
            echo "$line"
        fi
    done < <(command git-wt "$@")

    local exit_code=$?

    if [ -n "$cd_path" ] && [ -d "$cd_path" ]; then
        cd "$cd_path" || return 1
    fi

    return $exit_code
}
