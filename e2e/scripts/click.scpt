on run argv
    do shell script "/opt/homebrew/bin/cliclick c:" & (item 1 of argv) & "," & (item 2 of argv)
end run
