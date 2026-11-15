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

awake-tip = Kliknij, aby ekran znów mógł zasnąć

# The month in a date is in the genitive: "14 listopada", not the
# "Listopad" the month's heading says.
date = { $weekday ->
    [monday] poniedziałek
    [tuesday] wtorek
    [wednesday] środa
    [thursday] czwartek
    [friday] piątek
    [saturday] sobota
   *[sunday] niedziela
}, { $day } { $month ->
    [january] stycznia
    [february] lutego
    [march] marca
    [april] kwietnia
    [may] maja
    [june] czerwca
    [july] lipca
    [august] sierpnia
    [september] września
    [october] października
    [november] listopada
   *[december] grudnia
} { $year }

january = Styczeń
february = Luty
march = Marzec
april = Kwiecień
may = Maj
june = Czerwiec
july = Lipiec
august = Sierpień
september = Wrzesień
october = Październik
november = Listopad
december = Grudzień
monday-short = Pn
tuesday-short = Wt
wednesday-short = Śr
thursday-short = Cz
friday-short = Pt
saturday-short = So
sunday-short = Nd
