### Słowa panelu po polsku.
### Translated by a machine, and not yet read by someone who speaks Polish.

aeroplane-mode = Tryb samolotowy

# A list after a colon agrees with no radio's gender, as "wyłączony" or
# "wyłączone" after it would have to.
radios-off = Wyłączone: { $radios }

and = { $first } i { $last }

radio = { $kind ->
    [wlan] Wi-Fi
    [bluetooth] Bluetooth
    [uwb] UWB
    [wimax] WiMAX
    [wwan] Internet mobilny
    [gps] GPS
    [fm] Radio FM
    [nfc] NFC
   *[other] Radio
}

awake = Nie zasypia
awake-until = Nie zasypia do { $at }
