prefs-window-title = Preferences
prefs-general-page = General
prefs-vp-page = Video Player

# General
# =======

prefs-general-language =
    .title = Language
    .subtitle = Contribute to translations on <a href="{ $weblateUrl }">Weblate</a>
    .option-default = System Default ({ $languageId })

prefs-general-theme =
    .title = Theme
    .option-default = System Default
    .option-light = Light
    .option-dark = Dark

prefs-general-restore-most-recent =
    .title = Sign in automatically when { app-name } starts
    .subtitle = On startup, you will be signed in to your most recently used account

prefs-general-use-episode-image =
    .title = Use episode images in Next Up and Continue Watching sections
    .subtitle = Next Up and Continue Watching sections will use episode images as thumbnails instead of the primary thumbnail of the show.

# Interface
# =========

prefs-vp-interface = Interface
prefs-skip-amount =
    { $seconds } { $seconds ->
        [one] second
       *[other] seconds
    }
prefs-vp-skip-backwards =
    .title = Skip backwards amount
    .subtitle = How many seconds to skip backwards at a time
prefs-vp-skip-forwards =
    .title = Skip forwards amount
    .subtitle = How many seconds to skip forwards at a time
prefs-vp-on-left-click =
    .title = When the mouse is clicked
    .subtitle = What happens when you left click inside the video player
prefs-vp-on-left-click-options =
    .play-pause = Play/pause video
    .toggle-controls = Show/hide controls

# Subtitles
# =========

prefs-vp-subs = Subtitles
prefs-vp-subs-reset =
    .label = Reset
    .tooltip = Reset subtitle settings

prefs-vp-subs-style = Subtitle style
prefs-vp-subs-style-ass-warning =
    These settings don't apply to ASS/SSA subtitles.
    For ASS/SSA subtitles, see "{prefs-vp-subs-more}".

prefs-vp-subs-colour =
    .title = Subtitle text color
prefs-vp-subs-background-colour =
    .title = Subtitle background color
prefs-vp-subs-font =
    .title = Subtitle font
    .subtitle = Only supported fonts are listed

prefs-vp-subs-more = More subtitle preferences
prefs-vp-subs-more-ass-warning =
    .title = These preferences will affect ASS/SSA subtitles!
    .subtitle =
        ASS/SSA subtitles often include special styling and positioning. Changing these preferences may cause ASS/SSA subtitles to render incorrectly.

        If you only want to change the style for other subtitle formats, it's recommended that you change the preferences under "{prefs-vp-subs-style}" instead.

prefs-vp-subs-scale =
    .title = Subtitle scale
    .subtitle = Scaling factor for subtitle text
prefs-vp-subs-position =
    .title = Subtitle position
    .subtitle = Where 0 is the top of the screen, and 100 is the bottom

# Plugins
# =======

prefs-vp-plugins = Plugins
prefs-vp-intro-skipper =
    .title = Intro Skipper plugin
    .subtitle =
        Shows a Skip Intro button during episode intros.
        This requires the <a href="{ $introSkipperUrl }">Intro Skipper</a> plugin to be installed on your server.
prefs-vp-intro-skipper-auto-skip =
    .title = Automatically skip intros
    .subtitle = Skip intros without having to press the Skip Intro button
prefs-vp-jellyscrub =
    .title = Jellyscrub plugin
    .subtitle =
        Show thumbnails while hovering over the video progress bar.
        This requires the <a href="{ $jellyscrubUrl }">Jellyscrub</a> plugin to be installed on your server.
        Jellyfin's native trickplay will be used instead on supported servers.

# Other
# =====

prefs-vp-other = Other
prefs-vp-mpv-hwdec =
    .title = Hardware decoding
    .subtitle =
        Hardware decoding can improve playback performance.
        Choose "{ prefs-vp-mpv-hwdec-options.auto-safe }" or "{ prefs-vp-mpv-hwdec-options.off }" if you're experiencing issues.
        For more info, refer to the <a href="{ $mpvManualUrl }">MPV manual</a>.
prefs-vp-mpv-hwdec-options =
    .auto = Auto
    .auto-copy = Auto (copy)
    .auto-safe = Auto (safe)
    .custom = Custom
    .off = Off
prefs-vp-experimental =
    .title = Experimental preferences
    .subtitle =
        These preferences may be broken or incomplete.
        Modifying them is not recommended.
prefs-vp-backend =
    .title = Video player backend
    .subtitle = Requires restarting { app-name }.
    .value-mpv = MPV
    .value-gstreamer = GStreamer
prefs-vp-hls-playback =
    .title = HLS playback
    .subtitle = This may break audio and subtitle track selection.
prefs-account-page = Server & Account
prefs-account-server = Server
prefs-account-checking = checking …
prefs-account-connected = connected
prefs-account-unreachable = unreachable
prefs-account-switch-server = Switch server
prefs-account-account = Account
prefs-account-switch-account = Switch account
prefs-account-sign-out = Sign out
prefs-account-admin = Administrator
prefs-account-user = User
prefs-account-can-delete = may delete media
prefs-account-qc = Sign in a new device
    .title = Sign in a new device
    .subtitle = Enter the code from the other device (Quick Connect)
    .button = Authorize
    .invalid = The code has 6 digits.
    .done = Device authorized.
    .failed = Authorization failed – code expired or wrong?
prefs-jf-save-failed = Saving on the server failed.
prefs-jf-synced = Jellyfin – also applies in the web client
prefs-jf-any-language = Any
prefs-jf-playback-page = Playback
prefs-jf-tracks = Audio & Subtitles
prefs-jf-tracks-description = Stored in Jellyfin, also used by other clients. Applies on next playback.
prefs-jf-audio-language = Preferred audio language
prefs-jf-default-audio =
    .title = Play default audio track
    .subtitle = The track marked as default in the file, regardless of language
prefs-jf-subtitle-language = Preferred subtitle language
prefs-jf-subtitle-mode =
    .title = Subtitles
    .subtitle = When subtitles are turned on automatically
prefs-jf-subtitle-mode-default = Default (as flagged in the file)
prefs-jf-subtitle-mode-smart = Smart (only for foreign audio)
prefs-jf-subtitle-mode-forced = Only forced
prefs-jf-subtitle-mode-always = Always
prefs-jf-subtitle-mode-none = None
prefs-jf-remember-audio =
    .title = Remember audio selection
prefs-jf-remember-subtitles =
    .title = Remember subtitle selection
prefs-jf-remember =
    .subtitle = Jellyfin remembers the choice per series (web client and other clients)
prefs-jf-flow = Flow
prefs-jf-next-episode =
    .title = Play next episode automatically
    .subtitle = Jellyfin – continue with the next episode at the end
prefs-jf-quality =
    .title = Default quality
    .subtitle = When Rustfin starts; can be changed in the player
prefs-jf-segments = Segments
prefs-jf-segments-description = Intro, recap, credits and preview from the server's media segments.
prefs-jf-segment-skip = Skip automatically
prefs-jf-segment-ask = Show skip button
prefs-jf-segment-nothing = Do nothing
prefs-jf-segment-intro = Intro
prefs-jf-segment-recap = Recap
prefs-jf-segment-outro = Credits
prefs-jf-segment-preview = Preview
prefs-jf-home-page = Home
prefs-jf-home = Home screen
prefs-jf-home-description = Sections as in the web client (Jellyfin). Visible after "Refresh".
prefs-jf-hero =
    .title = Large "Continue watching" banner
    .subtitle = Rustfin only – at the top of the home screen
prefs-jf-section = Section { $number }
prefs-jf-section-my-media = My media
prefs-jf-section-my-media-small = My media (small)
prefs-jf-section-resume = Continue watching
prefs-jf-section-next-up = Next up
prefs-jf-section-latest = Latest media
prefs-jf-section-live-tv = Live TV
prefs-jf-section-recordings = Active recordings
prefs-jf-section-resume-audio = Continue listening
prefs-jf-section-resume-book = Continue reading
prefs-jf-section-none = None
prefs-jf-next-up = Next up
prefs-jf-max-days =
    .title = Max days back
    .subtitle = Jellyfin – older started shows drop out (0 = no limit)
prefs-jf-rewatching =
    .title = Allow rewatching
    .subtitle = Jellyfin – include already watched shows in "Next up"
prefs-jf-display = Display
prefs-jf-missing =
    .title = Show missing episodes
    .subtitle = Jellyfin – episodes that exist but are not on the server
prefs-jf-hide-played =
    .title = Hide watched items in "Latest media"
prefs-jf-libraries = Libraries
prefs-jf-libraries-description = Jellyfin – where a library appears. Unchecking "My media" also removes it from the tabs.
prefs-jf-library-my-media = My media
prefs-jf-library-latest = Latest
prefs-account-password =
    .title = Change password
    .subtitle = Jellyfin password of this account
    .button = Change
    .current = Current password
    .new = New password
    .repeat = Repeat new password
    .cancel = Cancel
    .save = Save
    .mismatch = The new passwords do not match.
    .done = Password changed.
    .failed = Password not changed – current password wrong?
