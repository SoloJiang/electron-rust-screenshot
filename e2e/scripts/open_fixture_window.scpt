on run argv
    set htmlPath to item 1 of argv
    tell application "Safari"
        activate
        set doc to make new document
        set URL of doc to "file://" & htmlPath
        tell window 1
            set bounds to {200, 200, 600, 500}
        end tell
    end tell
    delay 1
end run
