on run argv
    set x1 to item 1 of argv
    set y1 to item 2 of argv
    set x2 to item 3 of argv
    set y2 to item 4 of argv
    do shell script "/opt/homebrew/bin/cliclick dd:" & x1 & "," & y1 & " dm:" & x2 & "," & y2 & " du:" & x2 & "," & y2
end run
