### The panel's words, in English, which every other language falls back
### on a message at a time (ADR 79).

# Every radio of the machine is off.
aeroplane-mode = Aeroplane mode

# Some of the radios are off. $radios names them, as "Bluetooth", or
# "Wi-Fi, Bluetooth and NFC", and $count says how many there are.
radios-off = { $radios } off

# The last two of several things named, joined; those before them are
# joined to them by commas.
and = { $first } and { $last }

# A kind of radio, by $kind as the system calls it.
radio = { $kind ->
    [wlan] Wi-Fi
    [bluetooth] Bluetooth
    [uwb] Ultra-wideband
    [wimax] WiMAX
    [wwan] Mobile broadband
    [gps] GPS
    [fm] FM radio
    [nfc] NFC
   *[other] A radio
}

# The session is kept awake, as the user asked: for as long as it is, or
# until $at, a time of day as the clock shows it.
awake = Awake
awake-until = Awake until { $at }
