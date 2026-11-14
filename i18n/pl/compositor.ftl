### Własne słowa kompozytora po polsku.
### Translated by a machine, and not yet read by someone who speaks Polish.

close = Zamknij
maximize = Maksymalizuj
minimize = Minimalizuj
fullscreen = Pełny ekran
tile = Ułóż obok
left-half = Lewa połowa
right-half = Prawa połowa
top-half = Górna połowa
bottom-half = Dolna połowa
shade = Zwiń
keep-above = Zawsze na wierzchu
stick = Na wszystkich obszarach
send = Przenieś na obszar
new-workspace = Nowy obszar roboczy
beside = Obok: { $workspace }

choose-place = Wybierz, gdzie ma się znaleźć to okno
placed = { $arrangement }: to okno { $part }
two-side-by-side = Dwa obok siebie
one-above-the-other = Jedno nad drugim
one-beside-two = Jedno obok dwóch
two-beside-one = Dwa obok jednego
one-above-two = Jedno nad dwoma
two-above-one = Dwa nad jednym
four-quarters = Cztery ćwiartki
on-the-left = po lewej
on-the-right = po prawej
at-the-top = u góry
at-the-bottom = na dole
at-the-top-left = w lewym górnym rogu
at-the-top-right = w prawym górnym rogu
at-the-bottom-left = w lewym dolnym rogu
at-the-bottom-right = w prawym dolnym rogu

screens-configured = Tak, jak w config.yaml
screens-mirrored = Lustrzane: każdy ekran pokazuje to, co { $first }
screens-alone = Tylko { $alone }: pozostałe są wyłączone
keep-screens = Zachować ten układ ekranów?
keep-or-back = Enter go zachowuje. Escape lub { $left } { $left ->
    [one] sekunda
    [few] sekundy
    [many] sekund
   *[other] sekundy
} przywraca poprzedni.
screens-kept = Ekrany zostają tak, jak są
screens-back = Ekrany wróciły do poprzedniego układu
only-one-screen = Jest tylko jeden ekran, więc może stać tylko w jeden sposób

not-held = nie można tu ponownie uruchomić kompozytora: nic by go nie uruchomiło, tak jak robi to omorfos-hold, gdy omorfos jest całą sesją
older = omorfos został zaktualizowany, ale nowa wersja się nie uruchamia, więc działa poprzednia: to, co aktualizacja miała naprawić, nie jest jeszcze naprawione
updated = omorfos został zaktualizowany do nowej wersji, która czekała, jak długo mogła, na chwilę, gdy sesja nie była używana
log-in-again = omorfos został zaktualizowany; nowa wersja zostanie użyta po ponownym zalogowaniu
waits-for-x11 = omorfos został zaktualizowany; nowa wersja zostanie przejęta, gdy programy napisane dla X11 zostaną zamknięte, albo po ponownym zalogowaniu
not-starting = nowa wersja omorfos, `{ $program }`, nie uruchamia się ({ $why }), więc działająca zostaje, dopóki nie zostanie zainstalowana inna
keeps-stopping = `{ $program }` ciągle się zatrzymuje i nie zostanie ponownie uruchomiony aż do ponownego zalogowania
locker-keeps-stopping = Program blokujący ekran ciągle się zatrzymuje przy zablokowanej sesji: z innego terminala wirtualnego `omorfosctl action lock` uruchomi go ponownie
no-locker = nic nie jest ustawione do blokowania sesji: wskaż program blokujący ekran jako `locker` w config.yaml, na przykład `locker: [swaylock, -f]`
x11-restarted = serwer X zatrzymał się i został uruchomiony ponownie; działające programy napisane dla X11 zostały zamknięte
x11-keeps-stopping = serwer X ciągle się zatrzymuje; programy napisane dla X11 nie będą działać aż do ponownego zalogowania
glass-solid = Szkło jest rysowane jako pełne: rozmycie tej karty graficznej { $why }

a-program = Program
every-key = { $program } ma wszystkie klawisze, gdy ma klawiaturę, także te pulpitu: Super+Escape je odbiera
refused = `{ $program }` chciał { $asked ->
    [capture] robić zdjęcia ekranu
    [pointer] śledzić wskaźnik
    [panel] umieścić panel na ekranie
   *[lock] zablokować sesję
} i nie otrzymał zgody; aby mu pozwolić, niech omorfos uruchamia go z `may: [{ $role }]`, z `autostart` w config.yaml albo z klawisza w keys.yaml
taking = { $program } robi zdjęcia { $window ->
    [yes] okna na tym ekranie
   *[no] tego ekranu
}
shown-through = { $program } widzi { $window ->
    [yes] okno na tym ekranie
   *[no] ten ekran
}, przez { $through }
records-microphone = { $program } nagrywa z mikrofonu
records-sound = { $program } nagrywa odtwarzany dźwięk
uses-camera = { $program } używa kamery
taken-ago = Przed chwilą robiono zdjęcia tego ekranu
sound-ago = Przed chwilą nagrywano dźwięk
camera-ago = Przed chwilą używano kamery

problems-heading = Problemy (kliknij, aby zamknąć)
problems-more = … i jeszcze { $hidden }

this-computer = Ten komputer
thing = { $kind ->
    [Processor] Procesor
    [Memory] Pamięć
    [Graphics] Grafika
    [Storage] Dyski
    [System] System
    [Screens] Ekrany
   *[other] { $kind }
}
credits = { $heading ->
    [built-in] Wbudowane
    [used-to-make-what-is-shipped] Użyte do zbudowania tego, co dostarczane
    [recommended-by-the-packages-not-shipped] Zalecane przez pakiety, niedostarczane
    [thanks] Podziękowania
   *[other] { $original }
}
version = Wersja { $version }
licence = Na licencji MIT
copy = Kopiuj
copied = Skopiowano

terminal = Terminal
run = Uruchom…
stay-awake = Nie usypiaj
reload-configuration = Wczytaj konfigurację
about-omorfos = O omorfos
quit = Zakończ
