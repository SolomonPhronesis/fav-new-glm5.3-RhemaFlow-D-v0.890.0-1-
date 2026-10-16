# Foot pedals, MIDI and Stream Deck (v0.835.0)

## Foot pedal
Most pedals send Page Down / Page Up. RhemaFlow already treats those as forward / back: run-sheet step if the run-sheet is running, otherwise slide, otherwise verse. Nothing to set up.

## MIDI (pad controller, MIDI foot switch)
Tools > Pedal / MIDI / Deck > Refresh devices > pick yours > Connect. To teach a button: choose what it should do in "Teach a button", then press the button. A few starter buttons are set up. Not tested with real hardware here.

## Stream Deck
Turn the web server on (Settings; note the PIN). In the Stream Deck app add a "Website" button, tick "Access in background", and use the address from Tools > Pedal / MIDI / Deck > "Addresses for each button", e.g.
`http://localhost:8787/cmd?c=next_verse&pin=1234`
From another computer replace `localhost` with the Mac's address.

## Second campus / helper operator page
Open `http://<mac address>:8787/operator` in any browser, type the PIN, and you get a button for every action plus what is on the screen. Only on a trusted network.
