on run argv
    do shell script "/opt/homebrew/bin/cliclick m:" & (item 1 of argv) & "," & (item 2 of argv)
end run
