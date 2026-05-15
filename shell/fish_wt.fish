# git-wt: standalone wt alias
function wt
    set -l cd_path ""

    for line in (command git-wt $argv)
        if string match -qr '^CD:(.+)$' -- $line
            set cd_path (string replace -r '^CD:(.+)$' '$1' -- $line)
        else
            echo $line
        end
    end

    if test -n "$cd_path" -a -d "$cd_path"
        cd $cd_path
    end
end
