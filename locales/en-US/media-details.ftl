media-details-refresh-button = Refresh
media-details-unnamed-item = Unnamed Item
media-details-unnamed-episode = Unnamed Episode
media-details-unnamed-season = Unnamed Season
media-details-play-button =
    { $resume ->
        [true] Resume
       *[false] Play
    }
    .with-episode-and-season =
        { $resume ->
            [true] Resume
           *[false] Play
        } S{ $seasonNumber }:E{ $episodeNumber }
    .next-episode = Play next episode
media-details-episode-list-empty = No episodes were found for this season.
media-details-years = { $startYear } – { $endYear }
    .until-present = { $startYear } – Present
    .present = Present
media-details-run-time = { $hours ->
        [0] { $minutes }m
        *[other] { $hours }h { $minutes }m
    }
media-details-season-tooltip =
    This season has { $unplayedItemCount } unplayed { $unplayedItemCount ->
        [one] episode
       *[other] episodes
    }
    .unknown-item-count = This season has unplayed episodes
media-details-watched = Watched
media-details-unwatched = Not watched
media-details-toggle-watched-error = Failed to mark { $type ->
        *[episode] episode
        [series] series
        [movie] movie
    } as { $watched ->
        [true] watched
        *[false] not watched
    }
media-details-backdrop-error = Failed to load media backdrop
media-details-cast = Cast & Crew
media-details-cast-director = Director
media-details-cast-writer = Writer
media-details-version = Version
media-details-season-mark-played = Mark season as watched
media-details-season-mark-unplayed = Mark season as unwatched
media-details-media-info = Media info
media-details-media-info-video = Video
media-details-media-info-audio = Audio
media-details-media-info-subtitle = Subtitles
media-details-media-info-file = File
media-details-media-info-external = external
media-details-media-info-audio-count = { $count ->
    [one] 1 audio track
   *[other] { $count } audio tracks
}
media-details-media-info-subtitle-count = { $count ->
    [one] 1 subtitle
   *[other] { $count } subtitles
}
media-details-play-from-start = From the start
media-details-ends-at = ends at { $time }
media-details-season-count = { $count ->
    [one] 1 season
   *[other] { $count } seasons
}
media-details-chapters = Chapters
media-details-episode-remaining = { $minutes } min left
media-details-media-info-playback = Playback
media-details-media-info-playback-direct = Direct (no transcoding)
media-details-media-info-playback-transcode = Server transcodes (→ 1080p)
