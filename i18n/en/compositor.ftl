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

## What the compositor says of itself, as a notice or among the problems.
not-held = the compositor cannot be started again here: nothing would start it, as omorfos-hold does where omorfos is the whole session
older = omorfos was updated, but the new version would not start, so the one before it is running: whatever the update was to fix is not fixed yet
updated = omorfos has been updated to its new version, which waited as long as it may for a moment the session was not in use
log-in-again = omorfos has been updated, and the new version is used once you log in again
waits-for-x11 = omorfos has been updated, and the new version is taken up once the programs written for X11 have closed, or once you log in again
# Its own new version, the file $program, does not start, for $why, in
# the words of what failed.
not-starting = the new version of omorfos, `{ $program }`, does not start ({ $why }), so the one running stays until another is installed
# A program that comes with omorfos, the file $program, keeps stopping.
keeps-stopping = `{ $program }` keeps stopping, and will not be started again until you log in
locker-keeps-stopping = The locker keeps stopping with the session locked: from another virtual terminal, `omorfosctl action lock` starts it again
no-locker = nothing is set to lock the session with: name a screen locker as `locker` in config.yaml, such as `locker: [swaylock, -f]`
x11-restarted = the X server stopped and has been started again; programs written for X11 that were running have closed
x11-keeps-stopping = the X server keeps stopping; programs written for X11 will not run until you log in again
# What blurs glass could not be had, for $why, in the words of what failed.
glass-solid = Glass is drawn solid: the blur of this graphics card { $why }

## What programs did, or were not let do.
# A program that is not named, or not named in a way that can be read.
a-program = A program
# $program asked for every key, and was given them.
every-key = { $program } has every key while it has the keyboard, the desktop's among them: Super+Escape takes them back
# $program asked to do what it may only if omorfos started it with the
# role $role, and was refused: $asked is which of these it asked.
refused = `{ $program }` asked to { $asked ->
    [capture] take pictures of the screen
    [pointer] follow the pointer
    [panel] put a panel on the screen
   *[lock] lock the session
} and was not let; to let it, have omorfos start it with `may: [{ $role }]`, from `autostart` in config.yaml or a key in keys.yaml
# What the mark of a screen being recorded says when the pointer rests on
# it: $program takes pictures of it, or of a window on it if $window is
# `yes`, or is shown them through $through.
taking = { $program } is taking pictures of { $window ->
    [yes] a window on this screen
   *[no] this screen
}
shown-through = { $program } is shown { $window ->
    [yes] a window on this screen
   *[no] this screen
}, through { $through }
records-microphone = { $program } is recording from the microphone
records-sound = { $program } is recording the sound that plays
uses-camera = { $program } is using the camera
taken-ago = Pictures of this screen were taken a moment ago
sound-ago = Sound was recorded a moment ago
camera-ago = The camera was used a moment ago

## The list of what is wrong, in the corner of the screen.
problems-heading = Problems (click to dismiss)
# How many more there are than there is room for, $hidden.
problems-more = … and { $hidden } more

## "About omorfos".
this-computer = This computer
# A kind of thing the computer has, by its $kind in English, which is
# said as it is where there is nothing for it here.
thing = { $kind ->
    [Processor] Processor
    [Memory] Memory
    [Graphics] Graphics
    [Storage] Storage
    [System] System
    [Screens] Screens
   *[other] { $kind }
}
# The heading of a table of what omorfos is made with, by $heading, its
# words joined by hyphens, and $original, as CREDITS.md writes it.
credits = { $heading ->
    [built-in] Built in
    [used-to-make-what-is-shipped] Used to make what is shipped
    [recommended-by-the-packages-not-shipped] Recommended by the packages, not shipped
    [thanks] Thanks
   *[other] { $original }
}
version = Version { $version }
licence = Under the MIT licence
# The button that copies what the computer is, as text for a bug report,
# which is in English; and what it says once it has.
copy = Copy
copied = Copied
