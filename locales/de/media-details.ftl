media-details-years = { $startYear } – { $endYear }
    .until-present = { $startYear } – heute
    .present = heute
media-details-refresh-button = Aktualisieren
media-details-unnamed-episode = Unbenannte Folge
media-details-unnamed-item = Unbenanntes Element
media-details-play-button =
    { $resume ->
        [true] Fortsetzen
       *[false] Abspielen
    }
    .with-episode-and-season =
        S{ $seasonNumber }:E{ $episodeNumber } { $resume ->
            [true] fortsetzen
           *[false] abspielen
        }
    .next-episode = Nächste Folge abspielen
media-details-season-tooltip =
    Du hast { $unplayedItemCount } { $unplayedItemCount ->
        [one] Folge
       *[other] Folgen
    } dieser Staffel noch nicht gesehen
    .unknown-item-count = Du hast einige Folgen dieser Staffel noch nicht gesehen
media-details-episode-list-empty = Diese Staffel enthält keine Folgen.
media-details-unnamed-season = Unbenannte Staffel
media-details-watched = Angesehen
media-details-run-time =
    { $hours ->
        [0] { $minutes }m
       *[other] { $hours }h { $minutes }m
    }
media-details-unwatched = Nicht gesehen
media-details-backdrop-error = Hintergrundbild konnte nicht geladen werden
media-details-toggle-watched-error =
    { $type ->
       *[episode] Die Folge
        [series] Die Serie
        [movie] Der Film
    } konnte nicht als { $watched ->
        [true] gesehen
       *[false] nicht gesehen
    } markiert werden
media-details-cast = Besetzung & Stab
media-details-cast-director = Regie
media-details-cast-writer = Drehbuch
media-details-version = Version
media-details-season-mark-played = Staffel als gesehen markieren
media-details-season-mark-unplayed = Staffel als ungesehen markieren
media-details-media-info = Medieninfo
media-details-media-info-video = Bild
media-details-media-info-audio = Ton
media-details-media-info-subtitle = Untertitel
media-details-media-info-file = Datei
media-details-media-info-external = extern
media-details-media-info-audio-count = { $count ->
    [one] 1 Tonspur
   *[other] { $count } Tonspuren
}
media-details-media-info-subtitle-count = { $count ->
    [one] 1 Untertitel
   *[other] { $count } Untertitel
}
