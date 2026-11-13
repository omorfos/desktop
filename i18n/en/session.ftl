### The words of the notices omorfos-session shows, in English, which
### every other language falls back on a message at a time (ADR 79).

# A level changed, shown with how full it is.
volume = Volume
muted = Muted
microphone-muted = Microphone muted
microphone-on = Microphone on
brightness = Brightness

# Every radio of the machine was turned off, or one was turned back on.
aeroplane-on = Aeroplane mode on
aeroplane-off = Aeroplane mode off
# One kind of radio, $radio as `radio` below calls it, was turned off or on.
radio-off = { $radio } off
radio-on = { $radio } on
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

# The machine slept before the screen was locked, and woke unlocked.
unlocked-title = The screen was not locked before sleep
unlocked-body = The machine went to sleep before the screen locker had locked the screen, so it woke unlocked. Check that `locker` in config.yaml names a screen locker, and that it starts quickly.
