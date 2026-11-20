vp-volume-mute-tooltip =
    { $muted ->
        [true] Stummschaltung aufheben
       *[false] Stummschalten
    }
vp-unnamed-track = Unbenannte Spur
vp-subtitle-track-tooltip =
    { $subtitlesAvailable ->
        [true] Untertitel
       *[false] Keine Untertitel verfügbar
    }
vp-subtitle-track-off = Aus
vp-backend-gst-track-name = { $displayName } – { $title }
vp-audio-track-tooltip = Tonspuren
vp-no-subtitles-available = Keine Untertitel verfügbar.
vp-audio-track-menu = Tonspur
vp-duration-tooltip =
    .total = Verbleibende Dauer anzeigen
    .remaining = Gesamtdauer anzeigen
vp-next-prev-episode-tooltip =
    { $direction ->
        [next] Nächste
       *[previous] Vorherige
    } Folge
vp-unnamed-item = Unbenanntes Element
vp-play-pause-tooltip =
    { $playing ->
        [true] Pause
       *[false] Abspielen
    }
vp-fullscreen-tooltip =
    { $enter ->
        [true] Vollbild
       *[false] Vollbildmodus beenden
    }
vp-skip-forwards-backwards-tooltip =
    { $seconds } { $seconds ->
        [one] Sekunde
       *[other] Sekunden
    } { $direction ->
        [forwards] vorspulen
       *[backwards] zurückspulen
    }
vp-subtitle-track-external = Externer Untertitel
vp-skip-intro =
    .manual = Vorspann überspringen
    .manual-recap = Rückblick überspringen
    .manual-outro = Abspann überspringen
    .manual-preview = Vorschau überspringen
    .auto = Überspringe Vorspann in { $seconds }…
vp-subtitle-track-menu = Untertitel
vp-next-up-starting =
    Nächste Folge beginnt in { $remaining } { $remaining ->
        [one] Sekunde
       *[other] Sekunden
    }…
vp-backend-mpv-track-name =
    .title-and-language = { $title } – { $language }
    .id-and-language = Track { $id } – { $language }
    .id = Track { $id }
vp-next-up-action =
    .play = Jetzt abspielen
    .hide = Ausblenden
vp-playback-speed-tooltip = Wiedergabegeschwindigkeit
vp-playback-speed-normal = Normal
vp-playback-speed-toast = Abspielgeschwindigkeit: { $speed }x
vp-chapters-tooltip = Kapitel
vp-chapter = Kapitel
vp-quality-tooltip = Qualität
vp-quality-auto = Automatisch
vp-quality-auto-server = Server 1080p
vp-quality-auto-direct = direkt
vp-quality-original = Original (direkt)
vp-mini-player-enter = Mini-Player (klein, immer im Vordergrund)
vp-mini-player-leave = Zurück zum normalen Fenster
syncplay-tooltip = Gemeinsam schauen
syncplay-error = SyncPlay-Fehler
syncplay-in-group = In Gruppe „{ $name }“
syncplay-hint = Abspielen, Pause und Spulen gelten für alle in der Gruppe.
syncplay-leave = Gruppe verlassen
syncplay-no-groups = Noch keine Gruppen.
syncplay-participants = { $count ->
    [one] 1 Person
   *[other] { $count } Personen
}
syncplay-default-name = Rustfin
syncplay-create = Gruppe erstellen
syncplay-joined = Gruppe „{ $name }“ beigetreten
syncplay-user-joined = { $name } ist dazugekommen
syncplay-user-left = { $name } ist gegangen
syncplay-left = Gruppe verlassen
syncplay-disconnected = SyncPlay-Verbindung getrennt
syncplay-group-missing = Gruppe gibt es nicht mehr
syncplay-access-denied = Jemand in der Gruppe hat keinen Zugriff auf diesen Titel
syncplay-short = Gemeinsam
syncplay-state-waiting = Warte auf alle in der Gruppe…
syncplay-state-waiting-short = warte…
syncplay-state-playing = läuft
syncplay-state-playing-toast = Läuft gemeinsam
syncplay-state-paused = pausiert
syncplay-state-paused-toast = Für die Gruppe pausiert
syncplay-someone = jemand anderes
syncplay-by-you = dir
syncplay-paused-by = Pausiert von { $name }
syncplay-played-by = Weiter durch { $name }
syncplay-seeked-by = Gesprungen durch { $name } – warte auf alle…
syncplay-leave-any-tooltip = Verlässt jede Gruppe, in der der Server dieses Gerät noch führt
syncplay-dissolve = Gruppe auflösen
syncplay-reconnecting = SyncPlay-Verbindung weg – verbinde neu…
