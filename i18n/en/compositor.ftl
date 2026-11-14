### The compositor's own words, in English, which every other language
### falls back on a message at a time (ADR 79).

## The menu of a window's titlebar.
close = Close
maximize = Maximize
minimize = Minimize
fullscreen = Fullscreen
tile = Tile
left-half = Left half
right-half = Right half
top-half = Top half
bottom-half = Bottom half
shade = Shade
keep-above = Keep above
stick = Show on every workspace
send = Send to workspace
new-workspace = New workspace
# To join the windows of a workspace made for them, by its $workspace name.
beside = Beside { $workspace }

## The pictures of where a window may go, under the titlebar's button.
choose-place = Choose where this window goes
# The part of an $arrangement the pointer is on, and where in it, $part.
placed = { $arrangement }: this window { $part }
two-side-by-side = Two side by side
one-above-the-other = One above the other
one-beside-two = One beside two
two-beside-one = Two beside one
one-above-two = One above two
two-above-one = Two above one
four-quarters = Four quarters
on-the-left = on the left
on-the-right = on the right
at-the-top = at the top
at-the-bottom = at the bottom
at-the-top-left = at the top left
at-the-top-right = at the top right
at-the-bottom-left = at the bottom left
at-the-bottom-right = at the bottom right

## The screens tried another way, by the display key.
screens-configured = As config.yaml has them
# Every screen shows what $first, the name of the first, shows.
screens-mirrored = Mirrored: every screen shows what { $first } shows
# The screen named $alone alone is on.
screens-alone = Only { $alone }: the others are off
keep-screens = Keep the screens like this?
# How many seconds are $left before they go back as they were.
keep-or-back = Enter keeps them. Escape, or { $left } { $left ->
    [one] second
   *[other] seconds
}, puts them back.
screens-kept = The screens are kept as they are
screens-back = The screens are back as they were
only-one-screen = There is only one screen, so it stands only one way
