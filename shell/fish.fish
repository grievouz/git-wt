# git-wt integration v0.1.0
function git --wraps=git
    if test "$argv[1]" = "wt"
        set -e argv[1]
        
        set -l cd_path ""
        
        # Capture only stdout, let stderr pass through (for interactive prompts)
        for line in (command git-wt $argv)
            if string match -qr '^CD:(.+)$' -- $line
                set cd_path (string replace -r '^CD:(.+)$' '$1' -- $line)
            else
                echo $line
            end
        end
        
        # Change directory if we got a CD directive
        if test -n "$cd_path" -a -d "$cd_path"
            cd $cd_path
        end
    else
        command git $argv
    end
end
