# Voice commands (v0.816.0)

Generated from `crates/rf-core/src/cmdtable.rs` - the table the app matches against and shows under **Voice commands**. 405 spoken phrases + spoken zoom percentages.

Rules: words may have up to two other words between them. Phrases marked *short only* fire only in a sentence of 9 words or fewer, so normal preaching cannot trigger them. A scripture reference in the same sentence beats navigation commands. Bare "stop" only stops reading aloud.

Zoom by number: `zoom 300`, `zoom to three hundred percent`, `zoom three` (= 300). Range 60-400. `max zoom` = 400, `half max zoom` = 200.


## StopReading

- "stop reading" - "stop read aloud" - "stop the reading" - "stop reading it" - "stop reader" - "cancel reading"
- "stop talking" - "stop speaking" - "be quiet" - "quiet please" - "stop it" - "stop the voice" - "stop voice" - "no more reading" - "thats enough" - "stop read" *(short only)*

## ReadAloud

- "read it" - "read that" - "read this" - "read the verse" - "read the verses" - "read aloud" - "read the scripture" - "read the text"
- "read it aloud" - "read out loud" - "read to us" - "read it out" - "please read" - "start reading" - "start read aloud" *(short only)*

## ReadFaster

- "read faster" - "speak faster" - "faster reading" - "speed up reading" - "read quicker" *(short only)*

## ReadSlower

- "read slower" - "speak slower" - "slower reading" - "slow down reading" - "read slowly" *(short only)*

## AutoRead(true)

- "auto read on" - "turn on auto read" - "enable auto read" - "auto reading on" - "start auto reading" - "read every verse" *(short only)*

## AutoRead(false)

- "auto read off" - "turn off auto read" - "disable auto read" - "auto reading off" - "stop auto reading" - "stop auto read" *(short only)*

## ExitFullscreen

- "exit full screen" - "exit fullscreen" - "close full screen" - "close fullscreen" - "leave full screen" - "leave fullscreen" - "normal screen" - "exit full" - "leave full" - "close full"
- "windowed mode" - "back to window" - "un full screen" - "minimize stage" *(short only)*

## Fullscreen

- "full screen" - "fullscreen"
- "go full screen" - "go fullscreen" - "make it full screen" - "full screen mode" - "enter full screen" - "maximize stage" - "maximize screen" - "project full screen" *(short only)*

## OpenStage

- "open stage" - "open the stage" - "show stage" - "show the stage" - "open stage window" - "open projector" - "open the projector" - "open screen" - "open the screen" - "launch stage" - "show the screen" - "open display" - "bring up the stage" - "turn on the stage" - "stage on" *(short only)*

## CloseStage

- "close stage" - "close the stage" - "hide stage" - "hide the stage" - "close stage window" - "close projector" - "close the projector" - "close display" - "turn off the stage" - "stage off" - "shut the stage" - "close the window" *(short only)*

## ClearStage

- "clear the screen" - "clear screen" - "clear the stage" - "clear stage" - "close the screen" - "close screen" - "take it off"
- "blank screen" - "blank the screen" - "black screen" - "go black" - "go blank" - "clear it" - "clear this" - "clear that" - "remove it" - "remove the verse" - "hide the verse" - "hide verse" - "take it down" - "clear the verse" - "empty screen" - "clear display" - "wipe screen" - "blank stage" - "black out" - "blackout" *(short only)*

## ZoomTo(ZOOM_MAX)

- "max zoom" - "maximum zoom" - "zoom max" - "zoom maximum" - "biggest text" - "largest text" - "full zoom" - "zoom all the way" *(short only)*

## ZoomTo(ZOOM_HALF_MAX)

- "half max zoom" - "half max" - "half maximum" - "half zoom" - "zoom half" - "zoom halfway" - "medium zoom" *(short only)*

## ZoomTo(ZOOM_MIN)

- "minimum zoom" - "min zoom" - "zoom min" - "zoom minimum" - "smallest text" *(short only)*

## ZoomReset

- "reset zoom" - "normal size" - "reset text"
- "zoom reset" - "default size" - "default zoom" - "zoom normal" - "normal zoom" - "original size" *(short only)*

## ZoomIn

- "zoom in" - "text bigger" - "bigger text" - "make it bigger" - "make the text bigger"
- "bigger font" - "larger text" - "increase size" - "enlarge text" - "zoom closer" - "magnify" - "make text larger" - "bigger please" - "increase zoom" - "zoom up" *(short only)*

## ZoomOut

- "zoom out" - "text smaller" - "smaller text" - "make it smaller" - "make the text smaller"
- "smaller font" - "reduce size" - "shrink text" - "decrease zoom" - "zoom down" - "make text smaller" - "smaller please" *(short only)*

## LastScripture

- "last scripture" - "that again" - "it again" - "show again" - "say again" - "that verse" - "that passage" - "same verse" - "same passage" - "back to that" - "back to same" - "back to this"
- "last passage" - "put it back" - "bring it back" *(short only)*

## NextVerse

- "next verse" - "next one"
- "next scripture" - "next line" - "go forward" - "forward one verse" - "one verse forward" - "following verse" - "verse after" *(short only)*

## NextChapter

- "next chapter"
- "following chapter" - "chapter forward" - "forward one chapter" - "chapter after" *(short only)*

## PrevChapter

- "previous chapter" - "last chapter" - "back chapter"
- "prior chapter" - "go back a chapter" - "chapter before" - "one chapter back" - "preceding chapter" *(short only)*

## PrevVerse

- "previous verse" - "last verse" - "back verse" - "prior verse" - "go back"
- "back one verse" - "one verse back" - "verse before" - "preceding verse" - "previous one" - "previous scripture" *(short only)*

## FirstVerse

- "first verse" - "top of chapter" - "start of chapter" - "beginning of chapter" *(short only)*

## ApproveQuote

- "approve" - "approve it" - "approve that" - "yes show it" - "yes approve" - "accept it" - "accept that" - "confirm it" - "show it" - "put it up" - "put it on screen" - "show that one" - "go with that" - "use that one" - "thats the one" *(short only)*

## DismissQuote

- "dismiss" - "dismiss it" - "dismiss that" - "reject it" - "reject that" - "no not that" - "not that one" - "wrong verse" - "wrong one" - "never mind" - "ignore that" - "skip it" - "skip that" - "cancel that" - "thats wrong" *(short only)*

## PickMatch(1)

- "option one" - "option 1" - "first match" - "first option" - "choice one" - "pick one" *(short only)*

## PickMatch(2)

- "option two" - "option 2" - "second match" - "second option" - "second one" - "choice two" - "pick two" *(short only)*

## PickMatch(3)

- "option three" - "option 3" - "third match" - "third option" - "third one" - "choice three" - "pick three" *(short only)*

## PickMatch(4)

- "option four" - "option 4" - "fourth match" - "fourth option" - "fourth one" - "choice four" *(short only)*

## PickMatch(5)

- "option five" - "option 5" - "fifth match" - "fifth option" *(short only)*

## AutoApprove(true)

- "auto approve on" - "turn on auto approve" - "enable auto approve" - "automatic approval on" - "auto stage quotes" - "auto approve quotes" *(short only)*

## AutoApprove(false)

- "auto approve off" - "turn off auto approve" - "disable auto approve" - "automatic approval off" - "wait for approval" - "ask me first" - "stop auto approving" *(short only)*

## ShowAllMatches(true)

- "show all matches" - "show all possible" - "show all scriptures" - "show all options" - "show every match" - "list all matches" *(short only)*

## ShowAllMatches(false)

- "show one match" - "only one match" - "show best match only" - "show top match" - "single match" *(short only)*

## StartRecording

- "start recording" - "begin recording"
- "record this" - "resume recording" - "start the recording" - "record service" *(short only)*

## StopRecording

- "stop recording" - "end recording"
- "pause recording" - "stop the recording" - "finish recording" *(short only)*

## StopListening

- "stop listening" - "pause listening"

## Help(true)

- "show commands" - "show help" - "voice commands" - "list commands" - "what can i say" - "show the commands" - "open help" *(short only)*

## Help(false)

- "hide commands" - "hide help" - "close help" - "close commands" - "close the help" - "hide the commands" *(short only)*

## NextSlide / PrevSlide (v0.815.0)

Only in short sentences, like "next verse". While a slide is on the stage, "next" and "next verse" step slides too.

- "next slide" - "forward one slide" - "advance slide" - "advance the slide" - "slide forward"
- "previous slide" - "back one slide" - "go back one slide" - "slide back" - "slide before" - "the previous slide"

## MediaPlay (v0.815.0)

- "play video" - "play the video" - "start video" - "start the video" - "play movie" - "play the movie" - "play the clip" - "play clip" - "resume video" - "resume the video" *(short only)*

## MediaPause (v0.815.0)

- "pause video" - "pause the video" - "pause movie" - "pause the movie" - "pause clip" - "pause the clip" - "freeze the video" *(short only)*

## MediaRestart (v0.815.0)

- "restart video" - "restart the video" - "replay video" - "replay the video" - "video from the start" - "restart the clip" *(short only)*

## v0.816.0: the next 20 (short sentences only)

| Say | Does |
|---|---|
| "read till verse 20", "read until verse twenty", "read to verse 17", "read up to verse thirty", "read through verse 25" | reads aloud from the verse on screen to that verse of the same chapter, and shows the range |
| "read the chapter", "read to the end", "read the rest" | same, to the end of the chapter |
| "final verse", "last verse of the chapter", "end of chapter" | jumps to the chapter's last verse ("last verse" alone still means the previous verse) |
| "in the KJV", "KJV", "KJV translation", "K J V", "king james version", "basic english", "B B E", "A S V", "american standard", "Y L T", "darby", "webster", "rotherham", "A K J V" | switches to that translation if it is installed; "N I V", "E S V", "N L T"... say it is not installed |
| "next translation", "previous translation" | cycles through the menu |
| "go dark", "screen off", "lights out" / "screen on", "lights on" | blackout on / off (the verse stays). Older words like "go black" and "blackout" still mean Clear |
| "show history" / "hide history" | the list of scriptures shown |
| "show the browser" / "hide the browser" | the Bible browser |
| "open media" / "close media" | the Media & slides window |
| "dismiss all", "clear the queue" | empties the review cards |
| "lower third", "green screen", "large print", "classic look" | how the congregation screen looks |
| "put the stage on screen two" (also "send it to screen two", "use screen three") | moves the stage to that monitor |
| "start a five minute countdown", "set a timer for ten minutes" | starts a countdown and shows it |
| "pause the timer", "resume the timer", "stop the timer" | timer control (stop also takes it off the screen) |
| "show the timer", "show the clock" | puts them on the screen |
| "show strongs" / "hide strongs", "what is the Greek for love", "hebrew for peace" | the Greek and Hebrew panel (see STRONGS.md) |

Speech engines write "KJV" as "K J V" or "k.j.v." and long names as words; all of those are understood. Long names
("king james", "american standard") only count in a short sentence, so a sermon sentence about the King James
Version changes nothing. Last mention wins ("from the KJV to the BBE" gives BBE).

