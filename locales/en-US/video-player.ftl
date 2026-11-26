vp-unnamed-item = Unnamed Item
vp-next-up-starting =
    Next episode starting in { $remaining } { $remaining ->
        [one] second
       *[other] seconds
    }…
vp-next-up-action =
    .play = Play now
    .hide = Hide
vp-fullscreen-tooltip =
    { $enter ->
        [true] Enter
       *[false] Exit
    } fullscreen
vp-next-prev-episode-tooltip =
    { $direction ->
        [next] Next
       *[previous] Previous
    } episode
vp-play-pause-tooltip =
    { $playing ->
        [true] Pause
       *[false] Play
    }
vp-duration-tooltip =
    .total = Swap to remaining time
    .remaining = Swap to total duration
vp-skip-forwards-backwards-tooltip =
    Skip { $direction ->
        [forwards] forwards
       *[backwards] backwards
    } { $seconds } { $seconds ->
        [one] second
       *[other] seconds
    }
vp-skip-intro =
    .manual = Skip Intro
    .manual-recap = Skip Recap
    .manual-outro = Skip Credits
    .manual-preview = Skip Preview
    .auto = Skipping intro in { $seconds }…
vp-audio-track-tooltip = Audio Tracks
vp-audio-track-menu = Audio Track
vp-subtitle-track-tooltip =
    { $subtitlesAvailable ->
        [true] Subtitle Tracks
       *[false] No Subtitle Tracks Available
    }
vp-subtitle-track-menu = Subtitle Track
vp-subtitle-track-external = External Subtitle Track
vp-subtitle-track-off = Off
vp-no-subtitles-available = No subtitles available.
vp-volume-tooltip = Volume: { $percent }%
vp-volume-mute-tooltip =
    { $muted ->
        [true] Unmute
       *[false] Mute
    }
vp-playback-speed-tooltip = Playback Speed
vp-playback-speed-normal = Normal
vp-playback-speed-toast = Playback speed: { $speed }x
vp-unnamed-track = Unnamed Track
vp-backend-mpv-track-name =
    .title-and-language = { $title } – { $language }
    .id-and-language = Track { $id } – { $language }
    .id = Track { $id }
vp-backend-gst-track-name = { $displayName } – { $title }
vp-menu-hwdec = Hardware Decoding
vp-menu-deinterlace = Deinterlacing
vp-menu-deinterlace-toast =
    .on = Deinterlacing: { vp-menu-deinterlace-options.on }
    .off = Deinterlacing: { vp-menu-deinterlace-options.off }
vp-menu-deinterlace-options =
    .on = On
    .off = Off
    .auto = Auto
vp-menu-debug = Debug
vp-menu-debug-copy-stream-url = Copy stream URL
vp-menu-debug-copy-stream-url-toast =
    .success = Stream URL copied to clipboard
    .fail = Failed to copy stream URL to clipboard
vp-menu-debug-mpv-props = MPV Properties
vp-chapters-tooltip = Chapters
vp-chapter = Chapter
vp-quality-tooltip = Quality
vp-quality-auto = Automatic
vp-quality-auto-server = server 1080p
vp-quality-auto-direct = direct
vp-quality-original = Original (direct play)
vp-mini-player-enter = Mini player (small, always on top)
vp-mini-player-leave = Back to normal window
syncplay-tooltip = Watch together
syncplay-error = SyncPlay error
syncplay-in-group = In group “{ $name }”
syncplay-hint = Play, pause and seeking are shared with everyone in the group.
syncplay-leave = Leave group
syncplay-no-groups = No groups yet.
syncplay-participants = { $count ->
    [one] 1 person
   *[other] { $count } people
}
syncplay-default-name = Rustfin
syncplay-create = Create group
syncplay-joined = Joined group “{ $name }”
syncplay-user-joined = { $name } joined
syncplay-user-left = { $name } left
syncplay-left = Left the group
syncplay-disconnected = SyncPlay connection lost
syncplay-group-missing = Group no longer exists
syncplay-access-denied = Someone in the group cannot access this title
syncplay-short = Together
syncplay-state-waiting = Waiting for everyone in the group…
syncplay-state-waiting-short = waiting…
syncplay-state-playing = playing
syncplay-state-playing-toast = Playing together
syncplay-state-paused = paused
syncplay-state-paused-toast = Paused for the group
syncplay-someone = someone else
syncplay-by-you = you
syncplay-paused-by = Paused by { $name }
syncplay-played-by = Resumed by { $name }
syncplay-seeked-by = { $name } jumped – waiting for everyone…
syncplay-leave-any-tooltip = Leave any group the server still has this device in
syncplay-dissolve = Dissolve group
syncplay-reconnecting = SyncPlay connection lost – reconnecting…
vp-subtitles-preparing = Preparing subtitles …
