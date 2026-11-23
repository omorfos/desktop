#!/bin/bash
# What scale other systems choose for a screen, and what omorfos's rule
# gives. The tables of two documents were worked out here, with the figures
# in their text that were worked out and not read, and both are held to it:
#
#   docs/reviews/2026-11-19-what-scale-other-systems-choose.md
#   docs/adr/0100-a-screens-scale-by-how-near-it-is-read.md
#
#   tools/scale-survey.sh          prints them, as the documents have them
#   tools/scale-survey.sh check    fails where a document says otherwise
#
# Each system's rule is its own code, or the rule its maker documents,
# written again here as it was read on 19 November 2026, at the place named
# above it. Before anything is printed or checked, each is held to what its
# own project holds the rule to (`held_to_their_own`), so that one that has
# drifted from its source fails here first. Nothing was run on the systems
# themselves.
#
# The sums are awk's, as the shell has no fractions, and holding the
# documents to them is the shell's. AWK names the awk to do them with, where
# it is not to be the machine's own: CI runs `check` with each of two.
# Nothing of omorfos needs this, to build, to test or to run; it needs bash
# and awk, as the gates do. Run from the top of the repository.
set -euo pipefail
# A decimal point is a point and a letter is its bytes, whatever language
# the machine is set to.
export LC_ALL=C

survey=docs/reviews/2026-11-19-what-scale-other-systems-choose.md
decision=docs/adr/0100-a-screens-scale-by-how-near-it-is-read.md
tab=$(printf '\t')

# Every sum, and each row and figure as its document writes it. Says them
# one to a line: the document, `row` or `figure`, and the text, with a tab
# between. Says nothing, and fails, if a rule is not as its own project
# has it.
sums() {
    "${AWK:-awk}" -v survey="$survey" -v decision="$decision" -f /dev/stdin <<'AWK'
function abs(number) {
    return number < 0 ? -number : number
}

function min(first, second) {
    return first < second ? first : second
}

function max(first, second) {
    return first > second ? first : second
}

function between(value, low, high) {
    return max(low, min(value, high))
}

# The nearest whole number, a half going away from nothing, as C rounds.
function nearest(number) {
    return number >= 0 ? int(number + 0.5) : -int(-number + 0.5)
}

# The least whole number that is not under this one, which is over nothing.
function ceiling(number,    whole) {
    whole = int(number)
    return whole < number ? whole + 1 : whole
}

# The greatest number that divides both.
function common_divisor(first, second) {
    return second == 0 ? first : common_divisor(second, first % second)
}

# From corner to corner of what is this much each way.
function diagonal(wide, tall) {
    return sqrt(wide * wide + tall * tall)
}

# A scale as the documents write it: 2, 1.5, 1.67.
function written(scale,    whole, text) {
    whole = nearest(scale)
    if (abs(scale - whole) < 1e-9) {
        return sprintf("%d", whole)
    }
    text = sprintf("%.2f", scale)
    sub(/0+$/, "", text)
    return text
}

# A small number as the documents write it.
function word(count) {
    if (count == 1) return "one"
    if (count == 4) return "four"
    if (count == 5) return "five"
    if (count == 8) return "eight"
    if (count == 11) return "eleven"
    return sprintf("%d", count)
}

# As a sentence begins.
function capital(text) {
    return toupper(substr(text, 1, 1)) substr(text, 2)
}

# The text, with other words where it has these.
function replaced(text, old, with,    at) {
    at = index(text, old)
    if (at == 0) return text
    return substr(text, 1, at - 1) with substr(text, at + length(old))
}

# What a document has, said as the shell below takes it: a row of one of
# its tables, or a figure in its text.
function row(document, text) {
    printf("%s\t%s\t%s\n", document, "row", text)
}

function figure(document, text) {
    printf("%s\t%s\t%s\n", document, "figure", text)
}

# --- A screen -----------------------------------------------------------------

# A screen is known from here on by its number: what the tables call it,
# what kind it is, its diagonal in inches, its pixels, and the size in
# millimetres it says it is.
function screen_saying(called, as, nominal, pixels_across, pixels_down, wide, tall) {
    screens++
    name[screens] = called
    kind[screens] = as
    inches[screens] = nominal
    across[screens] = pixels_across
    down[screens] = pixels_down
    width[screens] = wide
    height[screens] = tall
    return screens
}

# One that says the size its diagonal makes it, to the nearest millimetre
# each way, which is what the tables go by.
function screen(called, as, nominal, pixels_across, pixels_down,    along) {
    along = diagonal(pixels_across, pixels_down)
    return screen_saying(called, as, nominal, pixels_across, pixels_down,
        nearest(nominal * mm_to_the_inch * pixels_across / along),
        nearest(nominal * mm_to_the_inch * pixels_down / along))
}

# One of which its pixels and the size it says are all that is known.
function sized(pixels_across, pixels_down, wide, tall) {
    return screen_saying("", "monitor", 0, pixels_across, pixels_down, wide, tall)
}

function says_a_size(s) {
    return width[s] > 0 && height[s] > 0
}

# Pixels to the inch along the diagonal, by the size it says.
function density(s) {
    return diagonal(across[s], down[s]) / (diagonal(width[s], height[s]) / mm_to_the_inch)
}

# The six shapes that get written where a size should be, as mutter and sway
# list them.
function a_shape(wide, tall) {
    return (wide == 1600 && (tall == 900 || tall == 1000)) ||
        (wide == 160 && (tall == 90 || tall == 100)) ||
        (wide == 16 && (tall == 9 || tall == 10))
}

# --- omorfos before ADR 100: `Scale::for_screen` as it was ------------------
function omorfos_today(s,    quarters) {
    if (width[s] == 0) return 1
    quarters = nearest(across[s] * mm_to_the_inch / width[s] / 120 * 4)
    return min(max(quarters, 4), 16) / 4
}

# --- KDE: `chooseScale`, in
# https://github.com/KDE/kwin/blob/c24a5f4e85683de94108fc3ca6ff48ea726f5037/src/outputconfigurationstore.cpp
function kde(s,    aim, least, by_width, by_height, scale, whole) {
    if (height[s] < 3 || width[s] < 3) return 1
    if (kind[s] == "laptop") {
        # Its own screen, behind a lid. One with no lid aims at 130 and
        # keeps 720 units if it is over 90 mm the shorter way, as a tablet's
        # is, and at 135 and 360 if not, as a phone's: the documents have
        # neither.
        aim = 125
        least = 800
    } else {
        aim = height[s] > 500 ? 30.5 : 96
        least = 800
    }
    by_width = between(across[s] / (width[s] / mm_to_the_inch) / aim, 1,
        between(across[s] / least, 1, 3))
    by_height = between(down[s] / (height[s] / mm_to_the_inch) / aim, 1,
        between(down[s] / least, 1, 3))
    scale = nearest(100 * min(by_width, by_height) / 5) * 5 / 100
    if (scale < 1.2) return 1
    whole = nearest(scale)
    return abs(whole - scale) < 0.06 ? whole : scale
}

# --- GNOME: `calculate_scale` and `for_each_scale`, in
# https://github.com/GNOME/mutter/blob/9bc8cd8427a5aa9bf8976acaef4426ce09a08537/src/backends/meta-monitor.c

# The scales a panel divides by: fractions of up to `most_parts` parts. They
# are left in `scales`, the least first, and how many there are is said.
function gnome_scales(pixels_across, pixels_down, most_parts,    parts, count, scale, found, i) {
    split("", scales)
    found = 0
    for (parts = 1; parts <= most_parts; parts++) {
        for (count = parts; count <= 4 * parts; count++) {
            if ((pixels_across * parts) % count != 0 || (pixels_down * parts) % count != 0) {
                continue
            }
            if (common_divisor(count, parts) > 1) continue
            scale = count / parts
            if (int(pixels_across / scale) * int(pixels_down / scale) < 600 * 600) continue
            for (i = found; i >= 1 && scales[i] > scale; i--) {
                scales[i + 1] = scales[i]
            }
            scales[i + 1] = scale
            found++
        }
    }
    if (found == 0) {
        scales[1] = 1
        found = 1
    }
    return found
}

function gnome(s, fractions,    along, aim, wanted, count, best, i) {
    if (a_shape(width[s], height[s]) || !says_a_size(s)) return 1
    along = diagonal(width[s], height[s]) / mm_to_the_inch
    aim = along < 20 ? 135 : 110
    wanted = diagonal(across[s], down[s]) / along / aim
    if (!fractions) wanted -= 0.125
    count = gnome_scales(across[s], down[s], fractions ? 4 : 1)
    best = scales[1]
    for (i = 2; i <= count; i++) {
        if (abs(scales[i] - wanted) < abs(best - wanted)) best = scales[i]
    }
    return best
}

# --- niri: `guess_monitor_scale`, in
# https://github.com/YaLTeR/niri/blob/a855fd519f7341d378577da6a7b2763baeb70f79/src/utils/scale.rs
function niri(s,    along, aim, wanted, quarters, scale, best, found) {
    if (!says_a_size(s)) return 1
    along = diagonal(width[s], height[s]) / mm_to_the_inch
    aim = along < 20 ? 135 : 110
    wanted = diagonal(across[s], down[s]) / along / aim
    found = 0
    for (quarters = 4; quarters <= 16; quarters++) {
        scale = quarters / 4
        if (nearest(across[s] / scale) * nearest(down[s] / scale) < 800 * 480) continue
        if (!found || abs(scale - wanted) < abs(best - wanted)) {
            best = scale
            found = 1
        }
    }
    return found ? best : 1
}

# --- COSMIC: `calculate_scale` and `scale_from_dpi`, in
# https://github.com/pop-os/cosmic-comp/blob/abb6ebc49ba58daa14163872954cdef3f830a04a/src/backend/kms/device.rs
function cosmic_from_density(per_inch, over, lines,    percent, scale, most, least) {
    percent = nearest(per_inch / over * 100)
    # Up to the next half.
    scale = ceiling(percent / 50) * 50 / 100
    most = lines <= 1200 ? 1 : (lines <= 1600 ? 1.5 : 2)
    least = lines >= 2000 ? 2 : 1
    return max(min(scale, most), least)
}

function cosmic(s, own_socket,    lines, wide, tall, product, per_inch) {
    lines = min(across[s], down[s])
    if (!says_a_size(s)) return lines <= 1600 ? 1 : 2
    wide = width[s] / mm_to_the_inch
    tall = height[s] / mm_to_the_inch
    # As it is written there, where it is called the diagonal: the root of
    # the product of the squares, which is the product of the sides.
    product = sqrt(wide * wide * (tall * tall))
    per_inch = (across[s] / wide + down[s] / tall) / 2
    if (product < 20 || own_socket) return cosmic_from_density(per_inch, 144, lines)
    if (product >= 40 && tall >= 19) return lines <= 1200 ? 1 : 2
    return cosmic_from_density(per_inch, 120, lines)
}

# --- sway: `compute_default_scale`, in
# https://github.com/swaywm/sway/blob/1652c54b73f67df17b7b4ab0b0f7048204aa8104/sway/config/output.c
function sway(s) {
    if (down[s] < 1200 || !says_a_size(s)) return 1
    if (a_shape(width[s], height[s])) return 1
    if (across[s] / (width[s] / mm_to_the_inch) > 192 &&
        down[s] / (height[s] / mm_to_the_inch) > 192) return 2
    return 1
}

# --- Hyprland: `getDefaultScale` and what its caller does with it, in
# https://github.com/hyprwm/Hyprland/blob/49b566a9780413a2498b7e8b925337ddb41a41a7/src/output/Monitor.cpp
function divides(s, by) {
    return across[s] / by == nearest(across[s] / by) && down[s] / by == nearest(down[s] / by)
}

function hyprland(s,    along, scale, start, step) {
    along = diagonal(width[s], height[s]) / mm_to_the_inch
    # A screen that says no size is divided by nothing there, which comes
    # to more than any density.
    if (along == 0 || diagonal(across[s], down[s]) / along > 200) {
        scale = 2
    } else if (diagonal(across[s], down[s]) / along > 140) {
        scale = 1.5
    } else {
        scale = 1
    }
    if (divides(s, scale)) return scale
    start = nearest(scale * 120)
    for (step = 1; step < 90; step++) {
        if (divides(s, (start + step) / 120)) return (start + step) / 120
        if (divides(s, (start - step) / 120)) return (start - step) / 120
    }
    return nearest(start / 120)
}

# --- ChromeOS: `FindDeviceScaleFactor` and the rest, in
# https://github.com/chromium/chromium/blob/25ba7ca183aafe2247743fbc2c0f4102994d6d01/ui/display/manager/display_change_observer.cc
# with the first zoom of an external screen from `managed_display_info.cc`
# beside it, and the steps of zoom from `ui/display/types/display_constants.h`.

# A Chromebook's own screen.
function chromebook(per_inch, pixels_across, pixels_down) {
    if (pixels_across == 3000 && pixels_down == 2000) return 3000 / 1332
    if (pixels_across == 3200 && pixels_down == 1800) return 2
    if (pixels_across == 2160 && pixels_down == 1440) return 1.8
    if (per_inch >= 310) return 8 / 3
    if (per_inch >= 270) return 2.4
    if (per_inch >= 230) return 2
    if (per_inch >= 220) return 16 / 9
    if (per_inch >= 180) return 1.6
    if (per_inch >= 150) return 1.25
    return 1
}

# The steps of zoom for a screen of this many pixels or more the longer way.
function zooms_from(pixels, steps) {
    zoom_tables++
    zooms_at[zoom_tables] = pixels
    zooms_for[zoom_tables] = steps
}

function chromeos(s,    believed, per_inch, wanted, longer, table, steps, zooms, zoom, i) {
    believed = width[s] > 40 && height[s] > 30 && !(width[s] == 50 && height[s] == 40) &&
        !(width[s] == 160 && (height[s] == 90 || height[s] == 100))
    per_inch = believed ? mm_to_the_inch * across[s] / width[s] : 0
    if (kind[s] == "laptop") return chromebook(per_inch, across[s], down[s])
    if (per_inch == 0) return 1
    wanted = per_inch / 96
    longer = max(across[s], down[s])
    for (table = 1; table <= zoom_tables; table++) {
        if (longer >= zooms_at[table]) steps = split(zooms_for[table], zooms, " ")
    }
    for (i = 1; i <= steps; i++) {
        zooms[i] += 0
    }
    zoom = zooms[steps]
    if (wanted <= zooms[1]) {
        zoom = zooms[1]
    } else if (wanted < zooms[steps]) {
        for (i = 1; i < steps; i++) {
            if (wanted < zooms[i + 1]) {
                zoom = wanted - zooms[i] < zooms[i + 1] - wanted ? zooms[i] : zooms[i + 1]
                break
            }
        }
    }
    if (zoom == 1) return 1
    return longer / int(longer / zoom + 0.0005)
}

# For a screen of 50 inches or more, behind a flag that is off.
function chromeos_behind_its_flag(s,    along, wanted, closest, tried, i) {
    along = diagonal(width[s], height[s]) / mm_to_the_inch
    if (along < 50) return 1
    wanted = diagonal(across[s], down[s]) / along / 40
    tried[1] = 1
    tried[2] = 1.25
    tried[3] = 4 / 3
    tried[4] = 1.6
    tried[5] = 16 / 9
    tried[6] = 2
    tried[7] = 8 / 3
    closest = 1
    for (i = 1; i <= 7; i++) {
        if (abs(wanted - tried[i]) > abs(wanted - closest)) break
        if ((across[s] / tried[i]) % 1 < 1e-6 && (down[s] / tried[i]) % 1 < 1e-6) {
            closest = tried[i]
        }
    }
    return closest
}

# --- Windows: "Calculating the Windows Default Scale Factor and Effective
# Resolution", in
# https://learn.microsoft.com/en-us/windows-hardware/design/component-guidelines/guidance-for-rounded-display-bezels

# From how many inches a screen of this kind is read.
function reads_from(as) {
    if (as == "tablet") return 20
    if (as == "laptop") return 24.5
    if (as == "monitor") return 28
    if (as == "television") return 84
}

# The default by the documented tables. The zoom it came from is left in
# `windows_zoom`.
function windows(s,    lines, most, wanted) {
    windows_zoom = density(s) / 96 * reads_from(kind[s]) / 28
    lines = down[s]
    most = lines < 900 ? 1 : (lines < 1080 ? 1.25 : (lines < 1440 ? 1.5 : 2))
    if (lines >= 1800) most = 2.5
    wanted = windows_zoom < 1.2 ? 1 : (windows_zoom < 1.43 ? 1.25 : (windows_zoom < 1.78 ? 1.5 : 2))
    if (windows_zoom >= 2.32) wanted = 2.5
    return min(wanted, most)
}

# What the rule of one of `others` gives a screen.
function gives(whose, s) {
    if (whose == "KDE") return kde(s)
    if (whose == "GNOME") return gnome(s, 1)
    if (whose == "niri") return niri(s)
    if (whose == "COSMIC") return cosmic(s, kind[s] == "laptop")
    if (whose == "sway") return sway(s)
    if (whose == "Hyprland") return hyprland(s)
    if (whose == "ChromeOS") return chromeos(s)
    if (whose == "Windows") return windows(s)
}

# --- What each project holds its own rule to ---------------------------------

# What a rule gives is what its own project has it give, give or take this
# much, or a millionth where that is not said.
function expect(what, got, want, give_or_take) {
    if (give_or_take == "") give_or_take = 1e-6
    if (abs(got - want) > give_or_take) {
        printf("%s: %s, where its own project has %s\n", what, got, want) > "/dev/stderr"
        wrong++
    }
}

# omorfos's own tests of the rule before ADR 100: pixels across, millimetres
# across, as they were.
function omorfos_has(pixels_across, wide, want) {
    expect("omorfos " pixels_across " " wide, omorfos_today(sized(pixels_across, 1, wide, 1)), want)
}

# mutter's src/tests/monitor-scale-tests.c: pixels, the size said, and what is
# expected with fractions, held there to 0.2, and without, held to 0.01.
function mutter_has(pixels_across, pixels_down, wide, tall, with_fractions, whole,    s, what) {
    s = sized(pixels_across, pixels_down, wide, tall)
    what = pixels_across " by " pixels_down ", " wide " by " tall " mm"
    expect("mutter " what, gnome(s, 1), with_fractions, 0.2)
    expect("mutter, whole, " what, gnome(s, 0), whole, 0.01)
    mutter_cases++
}

# niri's own, in the file its rule is in.
function niri_has(pixels_across, pixels_down, wide, tall, want) {
    expect("niri " pixels_across " by " pixels_down ", " wide " by " tall " mm",
        niri(sized(pixels_across, pixels_down, wide, tall)), want)
    niri_cases++
}

# A screen as cosmic-comp's test has it, which cuts the size down to whole
# millimetres.
function as_cosmic_tests_it(own_socket, nominal, pixels_across, pixels_down,    shape, tall) {
    shape = pixels_across / pixels_down
    tall = nominal * mm_to_the_inch / sqrt(shape * shape + 1)
    return cosmic(sized(pixels_across, pixels_down, int(tall * shape), int(tall)), own_socket)
}

# cosmic-comp's `test_scale`: the diagonals, then what each resolution gets
# at every one of them.
function cosmic_has(own_socket, diagonals, gets,    inch, each, count, parts, i, j) {
    count = split(diagonals, inch, " ")
    parts = split(gets, each, " ")
    for (i = 1; i <= count; i++) {
        for (j = 1; j < parts; j += 3) {
            expect("cosmic-comp " inch[i] " in, " each[j] " by " each[j + 1],
                as_cosmic_tests_it(own_socket, inch[i] + 0, each[j] + 0, each[j + 1] + 0),
                each[j + 2] + 0)
        }
    }
}

# Chromium's list of the panels Chromebooks have had, `lcd_display_configs`:
# the diagonal, the pixels, and the scale each gets. Its test takes the
# density along the diagonal.
function chromium_has(nominal, pixels_across, pixels_down, want) {
    expect("Chromium " nominal " in, " pixels_across " by " pixels_down,
        chromebook(diagonal(pixels_across, pixels_down) / nominal, pixels_across, pixels_down),
        want)
    chromebook_panels++
}

# Every rule against its own project's tests. What does not agree is said,
# and counted in `wrong`.
function held_to_their_own(    own_socket, per_inch) {
    omorfos_has(1920, 527, 1)
    omorfos_has(2560, 597, 1)
    omorfos_has(3840, 597, 1.25)
    omorfos_has(1920, 302, 1.25)
    omorfos_has(2880, 302, 2)
    omorfos_has(3840, 344, 2.25)
    omorfos_has(3840, 0, 1)

    mutter_has(720, 1440, 65, 129, 1.5, 1)
    mutter_has(1080, 2280, 68, 144, 2.5, 2)
    mutter_has(1080, 2400, 64, 142, 2.5, 2)
    mutter_has(2560, 1600, 286, 179, 1.75, 2)
    mutter_has(2400, 1600, 303, 202, 1.5, 1)
    mutter_has(3840, 2400, 290, 180, 2.5, 2)
    mutter_has(3840, 2400, 300, 190, 2.5, 2)
    mutter_has(1920, 1080, 509, 286, 1, 1)
    mutter_has(3840, 2160, 509, 286, 1.75, 2)
    mutter_has(3840, 2160, 598, 336, 1.5, 1)
    mutter_has(3840, 2160, 708, 398, 1.25, 1)
    mutter_has(3840, 2160, 554, 312, 1.5, 1)
    mutter_has(3840, 2160, 522, 294, 1.75, 2)

    niri_has(720, 1440, 65, 129, 1.5)
    niri_has(1080, 2280, 68, 144, 2.5)
    niri_has(1080, 2400, 64, 142, 2.5)
    niri_has(2560, 1600, 286, 179, 1.75)
    niri_has(2400, 1600, 303, 202, 1.5)
    niri_has(3840, 2400, 290, 180, 2.5)
    niri_has(3840, 2400, 300, 190, 2.5)
    niri_has(1920, 1080, 509, 286, 1)
    niri_has(3840, 2160, 509, 286, 1.75)
    niri_has(3840, 2160, 598, 336, 1.5)
    niri_has(3840, 2160, 708, 398, 1.25)
    niri_has(3840, 2160, 554, 312, 1.5)
    niri_has(3840, 2160, 522, 294, 1.75)
    niri_has(2560, 1600, 340, 210, 1.5)
    niri_has(2560, 1440, 700, 390, 1)
    niri_has(2736, 1824, 260, 170, 2)
    niri_has(1920, 1080, 0, 0, 1)

    # A laptop's own socket, eDP or LVDS, which its test gives alike; and
    # the same panels on another, which it gives the same.
    for (own_socket = 1; own_socket >= 0; own_socket--) {
        cosmic_has(own_socket, "14", "3000 2000 2")
        cosmic_has(own_socket, "14 15.6 17.3", "1920 1080 1  2560 1440 1.5  3840 2160 2")
        cosmic_has(own_socket, "14 16", "1920 1200 1  2560 1600 1.5  3840 2400 2")
    }
    cosmic_has(0, "24", "1920 1080 1  2560 1440 1.5  3840 2160 2")
    cosmic_has(0, "27 32 38", "1920 1080 1  2560 1440 1  3840 2160 2")
    cosmic_has(0, "26 29", "2560 1080 1  3440 1440 1.5  5120 2160 2")
    cosmic_has(0, "34 45", "2560 1080 1  3440 1440 1  5120 2160 2")
    cosmic_has(0, "45 49 57", "3840 1080 1  5120 1440 1  7680 2160 2")
    cosmic_has(0, "40 42 48 55 65 77 83", "1920 1080 1  2560 1440 2  3840 2160 2")
    cosmic_has(0, "0", "1920 1080 1  2560 1440 1  3840 2160 2")

    chromium_has(9.7, 1536, 2048, 2)
    chromium_has(10, 1280, 800, 1.25)
    chromium_has(10.1, 1280, 800, 1)
    chromium_has(10.1, 1920, 1080, 1.6)
    chromium_has(10.1, 1920, 1200, 16 / 9)
    chromium_has(10.5, 1920, 1200, 1.6)
    chromium_has(11.6, 1366, 768, 1)
    chromium_has(11.6, 1280, 720, 1)
    chromium_has(11.6, 1920, 1080, 1.6)
    chromium_has(12, 1920, 1080, 1.6)
    chromium_has(12.1, 1280, 800, 1)
    chromium_has(12.2, 1920, 1200, 1.6)
    chromium_has(12.2, 1920, 1080, 1.6)
    chromium_has(12.3, 2560, 1440, 2)
    chromium_has(13, 1920, 1080, 1.25)
    chromium_has(13.1, 3840, 2160, 8 / 3)
    chromium_has(13.3, 1366, 768, 1)
    chromium_has(13.3, 1920, 1080, 1.25)
    chromium_has(13.3, 2256, 1504, 1.6)
    chromium_has(13.3, 3840, 2160, 8 / 3)
    chromium_has(13.5, 1920, 1080, 1.25)
    chromium_has(13.5, 1920, 1280, 1.25)
    chromium_has(13.5, 2256, 1504, 1.6)
    chromium_has(13.6, 2256, 1504, 1.6)
    chromium_has(14, 1366, 768, 1)
    chromium_has(14, 1920, 1080, 1.25)
    chromium_has(14, 1920, 1200, 1.25)
    chromium_has(14, 2560, 1600, 1.6)
    chromium_has(14, 3840, 2160, 8 / 3)
    chromium_has(15.6, 1366, 768, 1)
    chromium_has(15.6, 1920, 1200, 1)
    chromium_has(15.6, 1920, 1080, 1)
    chromium_has(15.6, 3840, 2160, 2.4)
    chromium_has(17, 1600, 900, 1)
    chromium_has(17, 1920, 1080, 1)
    chromium_has(17.3, 1920, 1080, 1)
    chromium_has(18.51, 1366, 768, 1)
    chromium_has(11, 2160, 1440, 1.8)
    chromium_has(12, 1366, 912, 1)
    chromium_has(12.3, 2400, 1600, 2)
    chromium_has(12.85, 2560, 1700, 2)
    chromium_has(12.3, 3000, 2000, 3000 / 1332)
    chromium_has(13.3, 3200, 1800, 2)
    chromium_has(19.5, 1600, 900, 1)
    chromium_has(21.5, 1920, 1080, 1)
    chromium_has(23.8, 1920, 1080, 1)

    # Microsoft's worked example: 2560 by 1600 at 12.4 inches, read from 24.5,
    # is 243.46 to the inch, a zoom of 2.2 and a default of 200%.
    per_inch = diagonal(2560, 1600) / 12.4
    expect("Microsoft's example, its density", per_inch, 243.46, 0.005)
    expect("Microsoft's example, its zoom", per_inch / 96 * 24.5 / 28, 2.2, 0.05)
    expect("Microsoft's example", windows(screen("", "laptop", 12.4, 2560, 1600)), 2)

    # KWin, sway and Hyprland keep no test of the rule to hold theirs to.
    # KWin's comment has its aim for a television give 200% on a 77-inch one
    # of 3840 by 2160: that is so of one that says it is 1600 by 900 mm.
    expect("KWin's comment", kde(screen_saying("", "television", 0, 3840, 2160, 1600, 900)), 2)
}

# --- The screens the documents go by -----------------------------------------

function the_screens() {
    screen("13.3 in, 1920 by 1080", "laptop", 13.3, 1920, 1080)
    screen("14 in, 1920 by 1080", "laptop", 14, 1920, 1080)
    screen("14 in, 1920 by 1200", "laptop", 14, 1920, 1200)
    screen("15.6 in, 1920 by 1080", "laptop", 15.6, 1920, 1080)
    screen("16 in, 1920 by 1200", "laptop", 16, 1920, 1200)
    screen("17.3 in, 1920 by 1080", "laptop", 17.3, 1920, 1080)
    screen("13.5 in, 2256 by 1504", "laptop", 13.5, 2256, 1504)
    screen("13.3 in, 2560 by 1600", "laptop", 13.3, 2560, 1600)
    screen("14 in, 2560 by 1600", "laptop", 14, 2560, 1600)
    screen("16 in, 2560 by 1600", "laptop", 16, 2560, 1600)
    screen("14 in, 2880 by 1800", "laptop", 14, 2880, 1800)
    screen("16 in, 3200 by 2000", "laptop", 16, 3200, 2000)
    screen("13.4 in, 3840 by 2400", "laptop", 13.4, 3840, 2400)
    screen("14 in, 3840 by 2400", "laptop", 14, 3840, 2400)
    screen("15.6 in, 3840 by 2160", "laptop", 15.6, 3840, 2160)
    screen_saying("the same, saying 340 by 190 mm", "laptop", 15.6, 3840, 2160, 340, 190)
    screen("16 in, 3840 by 2400", "laptop", 16, 3840, 2400)
    screen("17.3 in, 3840 by 2160", "laptop", 17.3, 3840, 2160)
    screen("21.5 in, 1920 by 1080", "monitor", 21.5, 1920, 1080)
    screen("23.8 in, 1920 by 1080", "monitor", 23.8, 1920, 1080)
    screen("27 in, 1920 by 1080", "monitor", 27, 1920, 1080)
    screen("23.8 in, 2560 by 1440", "monitor", 23.8, 2560, 1440)
    screen("27 in, 2560 by 1440", "monitor", 27, 2560, 1440)
    screen("31.5 in, 2560 by 1440", "monitor", 31.5, 2560, 1440)
    screen("34 in, 3440 by 1440", "monitor", 34, 3440, 1440)
    screen("49 in, 5120 by 1440", "monitor", 49, 5120, 1440)
    screen("23.8 in, 3840 by 2160", "monitor", 23.8, 3840, 2160)
    screen("27 in, 3840 by 2160", "monitor", 27, 3840, 2160)
    screen("31.5 in, 3840 by 2160", "monitor", 31.5, 3840, 2160)
    screen("42.5 in, 3840 by 2160", "monitor", 42.5, 3840, 2160)
    screen("27 in, 5120 by 2880", "monitor", 27, 5120, 2880)
    screen("31.5 in, 6016 by 3384", "monitor", 31.5, 6016, 3384)
    screen("32 in, 1920 by 1080", "television", 32, 1920, 1080)
    screen("43 in, 1920 by 1080", "television", 43, 1920, 1080)
    screen("43 in, 3840 by 2160", "television", 43, 3840, 2160)
    screen("55 in, 3840 by 2160", "television", 55, 3840, 2160)
    screen("65 in, 3840 by 2160", "television", 65, 3840, 2160)
    screen("77 in, 3840 by 2160", "television", 77, 3840, 2160)
    screen_saying("a television saying 1600 by 900 mm, 3840 by 2160", "television", 0, 3840, 2160,
        1600, 900)
    screen_saying("a screen saying no size, 3840 by 2160", "television", 0, 3840, 2160, 0, 0)
    screen_saying("a screen saying no size, 1920 by 1080", "television", 0, 1920, 1080, 0, 0)
    listed = screens

    # Further screens the decision's last table has, beside the survey's.
    screen("42 in, 3840 by 2160", "television", 42, 3840, 2160)
    screen("48 in, 3840 by 2160", "television", 48, 3840, 2160)
    screen("85 in, 7680 by 4320", "television", 85, 7680, 4320)
    screen_saying("a projector saying no size, 1920 by 1200", "television", 0, 1920, 1200, 0, 0)
    screen_saying("a screen saying 16 by 9 mm, 2560 by 1440", "television", 0, 2560, 1440, 16, 9)
    tabled = screens

    # The laptop the roadmap names, by its nominal size and in whole
    # centimetres, as the survey took it.
    named = screen("", "laptop", 15.6, 3840, 2160)
    named_in_centimetres = screen_saying("", "laptop", 15.6, 3840, 2160, 340, 190)
    # And as its panel says it is, read from its EDID on 23 November 2026:
    # 14 inches, 309 by 174 mm.
    measured = screen_saying("", "laptop", 14, 3840, 2160, 309, 174)

    # What is kept back: a 7-inch and a 6.8-inch screen, and one that says it
    # is 10 mm square.
    small_one = screen("", "laptop", 7, 1280, 800)
    smaller_one = screen("", "laptop", 6.8, 1920, 1080)
    lying_one = screen_saying("", "laptop", 0, 3840, 2160, 10, 10)
}

# The screen the survey's tables call that.
function the(called,    s) {
    for (s = 1; s <= listed; s++) {
        if (name[s] == called) return s
    }
    printf("no screen is called %s\n", called) > "/dev/stderr"
    exit 1
}

# How many of the survey's screens are of a kind.
function count_of(as,    s, count) {
    count = 0
    for (s = 1; s <= listed; s++) {
        if (kind[s] == as) count++
    }
    return count
}

# The named laptop's panel, were it to say it is this wide.
function of_width(wide) {
    return screen_saying("", "laptop", 15.6, 3840, 2160, wide, nearest(wide * 9 / 16))
}

function in_quarters(wanted) {
    return min(max(nearest(wanted * 4) / 4, 1), 4)
}

# --- ADR 100: the rule ----------------------------------------------------------

# What the rule goes by: what is aimed at, to the inch, near and on a desk;
# under how many inches along the diagonal a screen is read from near; over
# how many millimetres on its shorter side it is a television; how near 1 or
# 2 a sum is taken for them; the lines to a scale where there is no size to
# go by; and the units on the shorter side that are kept.
function the_rule(    i) {
    aim_near = 135
    aim_on_a_desk = 110
    near_under = 20
    a_television_over = 560
    whole_within = 0.15
    two_within_over = 0.35
    lines_to_a_scale = 1080
    kept_back_to = 720
    # The lines between one scale and the next, and a hundredth or so to
    # either side of each.
    line_at[1] = 1 + whole_within
    line_at[2] = 1.375
    line_at[3] = 1.625
    line_at[4] = 2 - whole_within
    line_at[5] = 2 + two_within_over
    line_at[6] = 2.75
    line_at[7] = 3.25
    line_at[8] = 3.75
    split("1.14 1.37 1.62 1.84 2.35 2.74 3.24 3.74", just_under, " ")
    split("1.16 1.38 1.63 1.86 2.36 2.76 3.26 3.76", just_over, " ")
    for (i = 1; i <= 8; i++) {
        just_under[i] += 0
        just_over[i] += 0
    }
}

# A quarter up to 2 and a half beyond; 1 or 2 where within 0.15 of them,
# and 2 up to 0.35 over it.
function rounded(wanted,    scale) {
    if (wanted >= 2) {
        scale = nearest(wanted * 2) / 2
        if (wanted <= 2 + two_within_over) scale = 2
    } else {
        scale = nearest(wanted * 4) / 4
        if (abs(wanted - 1) <= whole_within) scale = 1
        if (abs(wanted - 2) <= whole_within) scale = 2
    }
    return min(max(scale, 1), 4)
}

function kept_back(scale, lines) {
    while (scale > 1 && lines / scale < kept_back_to) {
        scale -= 0.25
    }
    return scale
}

# The scale. What the screen is taken for is left in `taken_for`, and its
# sum in `its_sum`, which is empty where it has none.
function guess(s,    lines, nothing_true, television, near) {
    lines = min(across[s], down[s])
    nothing_true = !says_a_size(s) || a_shape(width[s], height[s]) || a_shape(height[s], width[s])
    television = !nothing_true && min(width[s], height[s]) > a_television_over
    its_sum = ""
    if (nothing_true || television) {
        taken_for = nothing_true ? "saying nothing true" : "a television"
        return kept_back(min(max(int(lines / lines_to_a_scale), 1), 4), lines)
    }
    near = diagonal(width[s], height[s]) / mm_to_the_inch < near_under
    taken_for = near ? "read from near" : "a monitor"
    its_sum = density(s) / (near ? aim_near : aim_on_a_desk)
    return kept_back(rounded(its_sum), lines)
}

# A screen's sum, as the documents write one.
function sum_of(called) {
    guess(the(called))
    return sprintf("%.2f", its_sum)
}

# A sum and the scale it is rounded to, as the decision writes the two.
function is_rounded(wanted) {
    return sprintf("%.2f is %s", wanted, written(rounded(wanted)))
}

# How far a sum is from the nearest line, as a share of itself.
function from_a_line(wanted,    i, nearest_line) {
    nearest_line = abs(wanted - line_at[1]) / wanted
    for (i = 2; i <= 8; i++) {
        nearest_line = min(nearest_line, abs(wanted - line_at[i]) / wanted)
    }
    return nearest_line
}

# The rule, were it in quarters all the way, which the decision turned down.
function quarters_all_the_way(wanted,    whole) {
    whole = nearest(wanted)
    return abs(wanted - whole) <= whole_within ? whole : in_quarters(wanted)
}

# On how many rows of laptops a rule aimed there is under all of the four.
function under_the_four(aim, quarters_only,    s, i, mine, under, count) {
    count = 0
    for (s = 1; s <= listed; s++) {
        if (kind[s] != "laptop") continue
        mine = quarters_only ? in_quarters(density(s) / aim) : rounded(density(s) / aim)
        under = 1
        for (i = 1; i <= 4; i++) {
            if (mine >= gives(the_four[i], s) - 1e-9) under = 0
        }
        count += under
    }
    return count
}

# On how many rows of laptops and monitors the rule is within what the
# others give: the four, or all of them.
function rows_within(only_the_four,    s, i, theirs, least, most, mine, count) {
    count = 0
    for (s = 1; s <= listed; s++) {
        if (kind[s] == "television") continue
        for (i = 1; i <= (only_the_four ? 4 : rules); i++) {
            theirs = gives(only_the_four ? the_four[i] : others[i], s)
            if (i == 1 || theirs < least) least = theirs
            if (i == 1 || theirs > most) most = theirs
        }
        mine = guess(s)
        if (least - 1e-9 <= mine && mine <= most + 1e-9) count++
    }
    return count
}

# --- The survey ---------------------------------------------------------------

# Microsoft's "Table 1 Windows 8.1 Scaling Levels": the diagonal, the pixels,
# the density and the level it gives, and the kind its size suggests; against
# the rule it documents now.
function windows_8_1(nominal, pixels_across, pixels_down, per_inch, level, as,    now, same) {
    now = windows(screen("", as, nominal, pixels_across, pixels_down))
    if (nearest(now * 100) == level) {
        same = "the same"
        the_same++
    } else {
        same = sprintf("%d%%", int(now * 100))
    }
    row(survey, sprintf("| %g in, %d by %d | %d | %d%% | %.0f | %s, from a zoom of %.2f as a %s |",
        nominal, pixels_across, pixels_down, per_inch, level, per_inch / (level / 100),
        same, windows_zoom, as))
}

# What a Mac looks like by default, as reported: the diagonal, the panel, and
# the size it is drawn as.
function apple(called, nominal, pixels_across, pixels_down, looks_across, looks_down, \
    per_inch, scale) {
    per_inch = diagonal(pixels_across, pixels_down) / nominal
    scale = pixels_across / looks_across
    row(survey, sprintf("| %s | %d by %d | %.0f | %d by %d | %s | %.0f |",
        called, pixels_across, pixels_down, per_inch, looks_across, looks_down,
        written(scale), per_inch / scale))
}

# A Chromebook panel the survey shows, of Chromium's list above.
function chromebook_shown(nominal, pixels_across, pixels_down,    per_inch, scale) {
    per_inch = diagonal(pixels_across, pixels_down) / nominal
    scale = chromebook(per_inch, pixels_across, pixels_down)
    row(survey, sprintf("| %g in, %d by %d | %.0f | %s | %d by %d | %.0f |",
        nominal, pixels_across, pixels_down, per_inch, written(scale),
        ceiling(pixels_across / scale - 1e-6), ceiling(pixels_down / scale - 1e-6),
        per_inch / scale))
}

# A row of the laptop the roadmap names: who chooses a scale, what it leaves
# to the inch and across, and from how far that would be read.
function its_row(who, scale,    left) {
    left = density(named) / scale
    row(survey, sprintf("| %s | %s | %.0f | %.0f | %d cm |",
        who, written(scale), left, 3840 / scale, nearest(2688 / left * 2.54)))
}

function survey_rows(    s, i, cells, now, dagger, each, count, alike) {
    windows_8_1(10.6, 1920, 1080, 208, 150, "tablet")
    windows_8_1(10.6, 1366, 768, 148, 100, "tablet")
    windows_8_1(11.6, 1920, 1200, 195, 150, "laptop")
    windows_8_1(11.6, 1366, 768, 135, 100, "laptop")
    windows_8_1(13.3, 1920, 1200, 170, 150, "laptop")
    windows_8_1(13.3, 2560, 1440, 221, 200, "laptop")
    windows_8_1(13.3, 1366, 768, 118, 100, "laptop")
    windows_8_1(15.4, 1920, 1080, 143, 125, "laptop")
    windows_8_1(15.6, 3200, 1800, 235, 200, "laptop")
    windows_8_1(17, 1920, 1080, 130, 125, "laptop")
    windows_8_1(23, 3840, 2160, 192, 200, "monitor")
    windows_8_1(24, 2560, 1440, 122, 125, "monitor")

    apple("MacBook Air, 13.6 in", 13.6, 2560, 1664, 1470, 956)
    apple("MacBook Air, 15.3 in", 15.3, 2880, 1864, 1710, 1107)
    apple("MacBook Pro, 13.3 in", 13.3, 2560, 1600, 1440, 900)
    apple("MacBook Pro, 14.2 in", 14.2, 3024, 1964, 1512, 982)
    apple("MacBook Pro, 16 in, of 2019", 16, 3072, 1920, 1792, 1120)
    apple("MacBook Pro, 16.2 in", 16.2, 3456, 2234, 1728, 1117)
    apple("iMac and Studio Display, 27 in", 27, 5120, 2880, 2560, 1440)
    apple("a 4K monitor, 23.8 in", 23.8, 3840, 2160, 1920, 1080)
    apple("a 4K monitor, 27 in", 27, 3840, 2160, 1920, 1080)
    apple("a 4K monitor, 31.5 in", 31.5, 3840, 2160, 1920, 1080)

    chromebook_shown(11.6, 1920, 1080)
    chromebook_shown(12.3, 2400, 1600)
    chromebook_shown(13.3, 1920, 1080)
    chromebook_shown(13.5, 2256, 1504)
    chromebook_shown(14, 1920, 1080)
    chromebook_shown(14, 2560, 1600)
    chromebook_shown(15.6, 1920, 1080)
    chromebook_shown(13.3, 3840, 2160)
    chromebook_shown(14, 3840, 2160)
    chromebook_shown(15.6, 3840, 2160)

    # What each rule gives each screen: omorfos's as it is, then the others'
    # in their order, Windows's last.
    for (s = 1; s <= listed; s++) {
        cells = written(omorfos_today(s))
        for (i = 1; i < rules; i++) {
            cells = cells " | " written(gives(others[i], s))
        }
        if (says_a_size(s)) {
            now = windows(s)
            # 300% is reported where the zoom is this high, over the page's most.
            dagger = windows_zoom >= 2.8 ? "†" : ""
            row(survey, sprintf("| %s | %.0f | %s | %s%s | %.2f |",
                name[s], density(s), cells, written(now), dagger, windows_zoom))
        } else {
            row(survey, sprintf("| %s |  | %s |  |  |", name[s], cells))
        }
    }

    # The laptop the roadmap names.
    alike = ""
    count = split("GNOME COSMIC sway Hyprland", each, " ")
    for (i = 1; i <= count; i++) {
        if (gives(each[i], named) == 2 && gives(each[i], named_in_centimetres) == 2) {
            alike = alike (alike == "" ? "" : ", ") each[i]
        }
    }
    its_row("what suits the maintainer", 1.75)
    its_row("what will do", 2)
    its_row(alike, 2)
    its_row("niri (" written(niri(named_in_centimetres)) " if the panel says 340 mm)", niri(named))
    its_row("KDE (" written(kde(named_in_centimetres)) " if it says 340 mm)", kde(named))
    its_row("omorfos today (" written(omorfos_today(named_in_centimetres)) " if it says 340 mm)",
        omorfos_today(named))
    its_row("ChromeOS, which had this panel", chromeos(named))
    its_row("Windows", windows(named))
}

# What the survey's text says that was worked out here.
function survey_figures(    count, i, s, w, divides_by, in_between, widest, choose_2, today, \
    theirs, more_gnome, less_gnome, more_kde, less_kde, each, less, more, monitor, first_at, \
    second_at, third_at, narrowest, narrowest_by_niri, first_zoom, second_zoom, large, at_55) {
    figure(survey, sprintf("the %d screens of mutter's test", mutter_cases))
    figure(survey, sprintf("niri's %d;", niri_cases))
    figure(survey, sprintf("the %d LCD panels", chromebook_panels))
    figure(survey, sprintf("the same for %s:", word(the_same)))

    count = gnome_scales(3840, 2160, 4)
    divides_by = " "
    for (i = 1; i <= count; i++) {
        divides_by = divides_by written(scales[i]) " "
    }
    if (index(divides_by, " 1.5 ") && index(divides_by, " 1.67 ") &&
        index(divides_by, " 2 ") && index(divides_by, " 2.5 ") &&
        !index(divides_by, " 1.75 ") && !index(divides_by, " 2.25 ")) {
        figure(survey, "2160 divides by 1.5, 1.67, 2 and 2.5, and not by 1.75 or 2.25")
    }
    count = gnome_scales(2256, 1504, 4)
    in_between = ""
    for (i = 1; i <= count; i++) {
        if (scales[i] > 1 && scales[i] <= 2) {
            in_between = in_between (in_between == "" ? "" : " and ") written(scales[i])
        }
    }
    figure(survey, "1504 by " in_between " and nothing between")

    figure(survey, sprintf("would give the 55-inch television %s, the 65-inch %s and " \
        "the 77-inch %s",
        written(chromeos_behind_its_flag(the("55 in, 3840 by 2160"))),
        written(chromeos_behind_its_flag(the("65 in, 3840 by 2160"))),
        written(chromeos_behind_its_flag(the("77 in, 3840 by 2160")))))

    figure(survey, sprintf("%d by %d mm, and %.0f to the inch. By that size today's rule gives %s.",
        width[named], height[named], density(named), written(omorfos_today(named))))
    widest = 0
    for (w = 320; w < 360; w++) {
        if (omorfos_today(of_width(w)) == 2.5) widest = w
    }
    figure(survey, sprintf("gives its width as %d mm or less", widest))
    choose_2 = 0
    for (i = 1; i <= rules; i++) {
        if (gives(others[i], named) == 2) choose_2++
    }
    figure(survey, sprintf("%s of the %s that have a rule choose it", word(choose_2), word(rules)))

    more_gnome = 0
    less_gnome = 0
    more_kde = 0
    less_kde = 0
    for (s = 1; s <= listed; s++) {
        if (kind[s] != "laptop") continue
        today = omorfos_today(s)
        theirs = gnome(s, 1)
        if (today > theirs + 1e-9) more_gnome++
        if (today < theirs - 1e-9) less_gnome++
        theirs = kde(s)
        if (today > theirs + 1e-9) more_kde++
        if (today < theirs - 1e-9) less_kde++
    }
    figure(survey, sprintf("Of the %d rows of laptops above it chooses more than GNOME on %d " \
        "and %s, and more than KDE on %d and less on %s.",
        count_of("laptop"), more_gnome, less_gnome ? sprintf("%d", less_gnome) : "never less",
        more_kde, word(less_kde)))

    split("KDE GNOME Windows", each, " ")
    more = 0
    for (i = 1; i <= 3; i++) {
        less[i] = 0
    }
    for (s = 1; s <= listed; s++) {
        if (kind[s] != "monitor") continue
        today = omorfos_today(s)
        for (i = 1; i <= 3; i++) {
            theirs = gives(each[i], s)
            if (today < theirs - 1e-9) less[i]++
            if (today > theirs + 1e-9) more++
        }
    }
    monitor = the("27 in, 3840 by 2160")
    figure(survey, sprintf("Of the %d monitors it %s more than KDE, GNOME or Windows, and less " \
        "than them on %d, %d and %d: a 27-inch 4K monitor gets %s where they give %s, %s and %s.",
        count_of("monitor"), more ? "chooses" : "never chooses", less[1], less[2], less[3],
        written(omorfos_today(monitor)), written(kde(monitor)), written(gnome(monitor, 1)),
        written(windows(monitor))))

    figure(survey, sprintf("a density between %.0f and %.0f gives it 2, and one between %.0f and " \
        "%.0f gives 1.75",
        density(named) / 2.125, density(named) / 1.875, density(named) / 1.875,
        density(named) / 1.625))
    first_at = in_quarters(density(the("14 in, 2880 by 1800")) / 150)
    second_at = in_quarters(density(the("13.5 in, 2256 by 1504")) / 150)
    third_at = in_quarters(density(the("16 in, 2560 by 1600")) / 150)
    if (second_at == third_at) {
        figure(survey, sprintf("the 14-inch 2880 by 1800 at %s, and the 13.5-inch 2256 by 1504 " \
            "and the 16-inch 2560 by 1600 at %s", written(first_at), written(second_at)))
    }
    figure(survey, sprintf("which they leave %.2f and %.2f units across", 3840 / 1.75, 3840 / 2.25))

    narrowest = 0
    narrowest_by_niri = 0
    for (w = 359; w >= 320; w--) {
        if (omorfos_today(of_width(w)) == 2.25) narrowest = w
        if (niri(of_width(w)) == 2) narrowest_by_niri = w
    }
    figure(survey, sprintf("%d mm or more and 2.5 if it says less; by niri it is 2 if it says " \
        "%d mm or more and 2.25 if less", narrowest, narrowest_by_niri))
    windows(the("14 in, 1920 by 1080"))
    first_zoom = windows_zoom
    windows(the("17.3 in, 3840 by 2160"))
    second_zoom = windows_zoom
    figure(survey, sprintf("sits at a zoom of %.2f and the 17.3-inch 4K at %.2f",
        first_zoom, second_zoom))

    large = the("42.5 in, 3840 by 2160")
    figure(survey, sprintf("is %d mm tall and gets %s on a desk",
        height[large], written(kde(large))))
    at_55 = kde(the("55 in, 3840 by 2160"))
    figure(survey, sprintf("some %d by %d on KDE at 55 inches",
        nearest(3840 / at_55 / 10) * 10, nearest(2160 / at_55)))
    figure(survey, sprintf("at once on %d of the %d rows of laptops",
        under_the_four(160, 1), count_of("laptop")))
}

# --- The decision ---------------------------------------------------------------

function decision_rows(    s, i, scale, count, order, called, says) {
    for (s = 1; s <= listed; s++) {
        if (kind[s] == "television") continue
        scale = guess(s)
        row(decision, sprintf("| %s | %d by %d mm | %.0f | %.2f | %s | %s |",
            replaced(name[s], "saying 340 by 190 mm", "in whole centimetres"),
            width[s], height[s], density(s), its_sum, written(scale), written(omorfos_today(s))))
    }

    # The televisions, the survey's and those this table has besides: by
    # their size, and by their pixels where two are of a size, then those
    # that have none.
    count = 0
    for (s = 1; s <= tabled; s++) {
        if (kind[s] != "television" || inches[s] == 0) continue
        for (i = count; i >= 1 && (inches[order[i]] > inches[s] ||
            (inches[order[i]] == inches[s] && across[order[i]] > across[s])); i--) {
            order[i + 1] = order[i]
        }
        order[i + 1] = s
        count++
    }
    for (s = 1; s <= tabled; s++) {
        if (kind[s] == "television" && inches[s] == 0) {
            count++
            order[count] = s
        }
    }
    for (i = 1; i <= count; i++) {
        s = order[i]
        called = replaced(name[s], "saying 1600 by 900 mm", "saying a shape")
        called = replaced(called, "a screen saying no size", "a screen saying nothing")
        says = width[s] ? sprintf("%d by %d mm", width[s], height[s]) : "nothing"
        scale = guess(s)
        row(decision, sprintf("| %s | %s | %s | %s | %s |",
            called, says, taken_for, written(scale), written(omorfos_today(s))))
    }
}

# What the decision's text says that was worked out here.
function decision_figures(    monitor, w, first_at_2, last_at_2, widest_over, short, said, i, \
    aim, first_for, last_for, s, scale, today, are_smaller, are_larger, monitors_larger, \
    monitors_smaller, rows, near_a_line) {
    monitor = the("27 in, 3840 by 2160")
    figure(decision, sprintf("a 27-inch 3840 by 2160 at %s, where they give %s, %s and %s",
        written(omorfos_today(monitor)), written(kde(monitor)), written(gnome(monitor, 1)),
        written(windows(monitor))))
    figure(decision, sprintf("is taken at its word and given %s",
        written(omorfos_today(sized(3840, 2160, 160, 90)))))
    figure(decision, sprintf("its sum is %.2f at 135", density(measured) / aim_near))
    figure(decision, sprintf("Windows's 110 gives it %s, and KDE's 125 %s",
        written(rounded(density(measured) / 110)), written(rounded(density(measured) / 125))))

    first_at_2 = 0
    last_at_2 = 0
    for (w = 300; w < 400; w++) {
        if (guess(of_width(w)) == 2) {
            if (first_at_2 == 0) first_at_2 = w
            last_at_2 = w
        }
    }
    figure(decision, sprintf("2 for any width from %d to %d mm", first_at_2, last_at_2))

    widest_over = 0
    for (w = 300; w < 346; w++) {
        if (quarters_all_the_way(density(of_width(w)) / aim_near) == 2.25) widest_over = w
    }
    figure(decision, sprintf("would be 2.25 from %d mm down", widest_over))
    figure(decision, sprintf("and over it from %d mm down", widest_over))
    short = (2 + whole_within) / (density(named_in_centimetres) / aim_near) - 1
    figure(decision, sprintf("leaves that laptop %.0f%% from the line", short * 100))
    figure(decision, sprintf("whose sum is %.2f", density(the("14 in, 1920 by 1080")) / aim_near))

    said = "A sum of " is_rounded(just_under[1]) ", and one of " is_rounded(just_over[1]) ". "
    for (i = 2; i <= 8; i++) {
        said = said is_rounded(just_under[i]) " and " is_rounded(just_over[i]) (i < 8 ? "; " : "")
    }
    figure(decision, said ", as is anything over.")
    said = "at "
    for (i = 1; i < 8; i++) {
        said = said sprintf("%g%s", line_at[i], i < 7 ? ", " : "")
    }
    figure(decision, said sprintf(" and %g", line_at[8]))

    scale = guess(small_one)
    figure(decision, sprintf("A 7-inch 1280 by 800, whose sum is %.1f, stays at %s, as 1.25 " \
        "would leave it %.0f units", its_sum, written(scale), 800 / 1.25))
    scale = guess(smaller_one)
    figure(decision, sprintf("A 6.8-inch 1920 by 1080, whose sum is %.2f, gets %s, which leaves " \
        "%.0f by %.0f", its_sum, written(scale), 1920 / scale, 1080 / scale))
    figure(decision, sprintf("is 10 mm square gets %s", written(guess(lying_one))))

    first_for = 0
    last_for = 0
    for (aim = 120; aim < 200; aim++) {
        if (rounded(density(named) / aim) == 1.75) {
            if (first_for == 0) first_for = aim
            last_for = aim
        }
    }
    figure(decision, sprintf("which is one from %d to %d", first_for, last_for))
    figure(decision, sprintf("at once on %d of the %d rows of screens under 20 inches; at 150 " \
        "on %d; at 140 on %d; at 135 on %s",
        under_the_four(160, 0), count_of("laptop"), under_the_four(150, 0), under_the_four(140, 0),
        word(under_the_four(135, 0))))
    figure(decision, sprintf("by KDE's rule %s at 55 inches and %s at 77",
        written(kde(the("55 in, 3840 by 2160"))), written(kde(the("77 in, 3840 by 2160")))))

    are_smaller = 0
    are_larger = 0
    monitors_larger = 0
    monitors_smaller = 0
    for (s = 1; s <= listed; s++) {
        scale = guess(s)
        today = omorfos_today(s)
        if (kind[s] == "laptop") {
            if (scale < today - 1e-9) are_smaller++
            if (scale > today + 1e-9) are_larger++
        }
        if (kind[s] == "monitor") {
            if (scale < today - 1e-9) monitors_smaller++
            if (scale > today + 1e-9) monitors_larger++
        }
    }
    figure(decision, sprintf("%d of the %d rows above are smaller and %s larger",
        are_smaller, count_of("laptop"), are_larger ? sprintf("%d", are_larger) : "none"))
    if (monitors_larger == 4 && monitors_smaller == 0) {
        figure(decision, sprintf("of the %d monitors, the 23.8- and 27-inch 3840 by 2160 and the " \
            "two denser still are larger, and none smaller", count_of("monitor")))
    }

    rows = count_of("laptop") + count_of("monitor")
    if (rows_within(0) == rows && rows_within(1) == rows - 1) {
        figure(decision, sprintf("All %d rows are within what the survey's %s other rules give, " \
            "and all but one within KDE, GNOME, ChromeOS and Windows.", rows, word(rules)))
    }
    near_a_line = 0
    for (s = 1; s <= listed; s++) {
        if (kind[s] != "laptop") continue
        guess(s)
        if (its_sum > 1 && from_a_line(its_sum) < 0.02) near_a_line++
    }
    figure(decision, sprintf("%s of the %d rows are within 2%% of one",
        capital(word(near_a_line)), count_of("laptop")))
    figure(decision, sprintf("at %s by the line at 1.15; the 14- and 16-inch 2560 by 1600, at " \
        "%s and %s by those at 1.625 and 1.375; the 17.3-inch 3840 by 2160, at %s by that " \
        "at 1.85; and the 14-inch 3840 by 2400, at %s by that at %s",
        sum_of("14 in, 1920 by 1080"), sum_of("14 in, 2560 by 1600"), sum_of("16 in, 2560 by 1600"),
        sum_of("17.3 in, 3840 by 2160"), sum_of("14 in, 3840 by 2400"), 2 + two_within_over))
}

BEGIN {
    mm_to_the_inch = 25.4
    # The others that have a rule, and the four of them most used.
    rules = split("KDE GNOME niri COSMIC sway Hyprland ChromeOS Windows", others, " ")
    split("KDE GNOME ChromeOS Windows", the_four, " ")
    zooms_from(0, "0.60 0.65 0.70 0.75 0.80 0.85 0.90 0.95 1.0")
    zooms_from(720, "0.70 0.75 0.80 0.85 0.90 0.95 1.0 1.05 1.10")
    zooms_from(800, "0.75 0.80 0.85 0.90 0.95 1.0 1.05 1.10 1.15")
    zooms_from(960, "0.90 0.95 1.0 1.05 1.10 1.15 1.20 1.25 1.30")
    zooms_from(1280, "0.90 1.0 1.05 1.10 1.15 1.20 1.25 1.30 1.50")
    zooms_from(1920, "1.0 1.10 1.15 1.20 1.30 1.40 1.50 1.75 2.00")
    zooms_from(3840, "1.0 1.10 1.20 1.40 1.60 1.80 2.00 2.20 2.40")
    zooms_from(5120, "1.0 1.25 1.50 1.75 2.00 2.25 2.50 2.75 3.00")

    the_rule()
    the_screens()
    held_to_their_own()
    if (wrong) exit 1
    survey_rows()
    survey_figures()
    decision_rows()
    decision_figures()
}
AWK
}

# What was worked out of one kind, `row` or `figure`, for a document.
its() {
    local document kind text
    while IFS="$tab" read -r document kind text; do
        if [ "$kind" = "$1" ] && [ "$document" = "$2" ]; then
            printf '%s\n' "$text"
        fi
    done <<<"$worked_out"
}

# Both documents, held to what was worked out: a row is a line of its
# document, and a figure is in its text, however the lines are broken there.
check() {
    local failed=0 document whole text rows figures missing
    for document in "$survey" "$decision"; do
        if [ ! -r "$document" ]; then
            echo "$document is not here: run from the top of the repository." >&2
            return 1
        fi
        missing=0
        rows=0
        while IFS= read -r text; do
            rows=$((rows + 1))
            if ! grep -qFx -e "$text" "$document"; then
                printf '%s: has no row\n    %s\n' "$document" "$text" >&2
                missing=1
            fi
        done < <(its row "$document")
        whole=$(tr -s '[:space:]' ' ' <"$document")
        figures=0
        while IFS= read -r text; do
            figures=$((figures + 1))
            case "$whole" in
                *"$text"*) ;;
                *)
                    printf '%s: does not say\n    %s\n' "$document" "$text" >&2
                    missing=1
                    ;;
            esac
        done < <(its figure "$document")
        if [ "$missing" -eq 0 ]; then
            echo "$document: $rows rows and $figures figures, as worked out here"
        else
            failed=1
        fi
    done
    return "$failed"
}

# Both documents' rows and figures, as they were worked out.
said() {
    local document
    for document in "$survey" "$decision"; do
        printf '%s\n\nIts rows, table after table:\n\n' "$document"
        its row "$document"
        printf '\nIts figures:\n\n'
        its figure "$document" | sed 's/^/- /'
        echo
    done
}

case "$*" in
    "" | check) ;;
    *)
        echo "usage: $0 [check]" >&2
        exit 2
        ;;
esac
worked_out=$(sums)
if [ "$*" = check ]; then
    check
else
    said
fi
