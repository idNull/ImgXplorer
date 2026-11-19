# imgxplorer — guide for AI video editors

You are editing an animated video from comic pages or images inside **imgxplorer**, the user's app. The user sees
every change live in the app and can undo it (Ctrl+Z, or the `undo` tool). Talk to the user in their language.

Images are addressed by **index** (0-based, from `list_images`). Times are **seconds**. Rectangles are **normalized**
(0..1 of the image: x, y = top-left corner, w, h = size).

Every tool that returns an image also saves it to disk and gives its path. If you cannot see the image in the tool
result, open that file with your own image viewing tool (for example Codex `view_image`, or reading the file).

## Workflow

1. **Open and understand.** `app_status`. If nothing is open, `open_source` with the path the user gives.
   `list_images`. Ask the user (or infer) the style: pace (calm / normal / action), video shape, music, voices.
2. **Video format first.** `set_output` (e.g. 1080p landscape for YouTube, portrait for phones). Framing depends on it.
3. **Look at every page.** `view_image` shows the page with numbered panels in reading order. Decide the reading
   direction: western comics `ltr`, manga `rtl`. Pass the same `direction` to `view_image`, `view_panel` and
   `plan_shots`. If panels are wrong (merged, missing, split), fix them with `set_panels` and look again.
4. **Read the panels.** `view_panel` each panel (or a region) at high resolution. Count the words in balloons and
   captions, notice the important details, the action and the mood.
5. **Plan the shots** of each image with `plan_shots` (see timing rules). Use panels for normal reading, `rect`
   for close-ups of a face, an object or one balloon, and `full` for splash pages or establishing shots.
6. **Review the animation.** `render_frames` gives a contact sheet of the real video frames (one per shot). Check
   that each shot shows the whole balloon (not cut), that the framing is good and the order is right. Fix and repeat.
7. **Transitions.** `set_transition` (default for all, or per image): `fade` between scenes, `cut` for fast action,
   `slide` or `continuous-camera` for page turns. `set_hold_between` for the pause between images.
8. **Sound.** See the sound section. `list_tracks`, `add_track` per character or kind of sound, `add_audio`,
   `add_ambient`, effects with `edit_track`. Then `sound_report` and fix problems.
   **Visual effects** where a panel calls for them (see the section): `add_fx`, then `render_frames` to check.
9. **Summary.** `summary` lists every image with warnings. Fix all warnings that matter.
10. **Ask the user to watch** with `play` (they can hear; you cannot) before exporting, when it makes sense.
11. **Export.** `export_video`, then poll `export_status` every few seconds until done.
12. **Check the MP4.** `inspect_video` with the times of a few key moments: duration, sound present, frames look
    right. Tell the user where the file is and what you did.

Work image by image and keep the user informed with short messages. For long comics, do the first page or two,
show the user (`render_frames`, `play`) and agree on the style before doing the rest.

## Timing rules (reading time)

Starting values; adapt to the user's pace (calm ×1.3, action ×0.7):

- **Hold per panel** = 1.5 s base + 0.3 s per word of text (balloons, captions, sound effects that matter).
- **No text but important art** (action, splash, emotional face): 2–3.5 s. Dense details: +1–2 s.
- **Minimum** 2 s for any shot the viewer must understand; 1–1.5 s only for quick action beats.
- **Long text** (more than ~30 words in a panel): split the panel into two or three shots with `rect` on each
  group of balloons, in reading order, instead of one very long hold.
- **Travel (`move`) between panels**: 0.6–1.2 s, `ease-in-out`. Shorter (0.3–0.5 s) for action; `move: 0` for a
  hard cut (surprise, impact).
- **Start**: `start: "overview"` with `overviewHold` 1–2 s on the first page of a scene to show the whole layout;
  `direct` for the following pages. **End**: `end: "overview"` only when the full page composition matters.
- If a voice clip is longer than the shot, make the hold longer: the shot should last at least as long as its line.

## Camera style

- Frame panels whole by default (`margin` 0.03–0.06). Never cut a balloon in half: check with `render_frames`.
- Close-ups: `rect` around a face or object, maybe `zoom` 1.1–1.3. Keep them short.
- Use `push` 1.03–1.08 for slow, cinematic push-ins on emotional or dramatic holds; 0.95 for pulling out.
- `bounce`/`elastic` easing only for comedy; `rot` almost never.
- Big moves across the page are fine, but not on every panel: vary the rhythm.

## Image adjustments

- `get_adjust` / `set_adjust` change how pages look without touching the files (for one image or, without
  `image`, for all). They show in the viewer, `render_frames` and the video.
- Use `clean: 1` for scans with yellowed or grey paper. Otherwise keep it subtle (±0.1–0.3); `tone: "sepia"` or
  `"gray"` can mark a flashback chapter. Don't adjust pages that already look good.

## Visual special effects

- `fx_catalog` lists effects and ready-made **situations**. A situation adds several synced effects and its
  built-in sound at once: `explosion`, `punch`, `shot`, `lightning`, `earthquake`, `shout`, `speed`, `surprise`
  (manga focus lines), `fear`, `flashback`, `dream`, `power`, `damage`. Prefer situations; use single effects to
  fine-tune.
- Use them **only where the panel shows it**: an explosion, a blow landing, a lightning strike, a character's shock,
  a running scene, a flashback page. Most panels need none. Never stack strong effects on every shot.
- Timing: `add_fx` with `shot` (from `plan_shots`, 1-based) and a small `offset` (0.1–0.4 s after the camera
  arrives), so the reader sees the panel first and then it hits. Put the matching sound at the same moment (situations
  do it for you; pass `sound: false` if the user already placed one).
- Point (`at`, normalized image coordinates): the center of the explosion, the fist, the face of the surprised
  character. Get it from `view_image` / `view_panel` (panel rects are normalized too).
- Impact effects (shake, flash, shockwave, impact frame) are short: 0.2–1 s. Sustained ones (color, vignette, wave,
  lines) can cover a whole shot; flashback color can cover the whole image.
- Flashes and negative frames are capped at 3 per second for photosensitive viewers; `summary` warns if too many
  happen close together. Keep strobing rare.
- Review with `render_frames` (it adds a frame at the peak of each effect) and `get_timeline` (effect ids for
  `edit_fx` / `remove_fx`).

## Onomatopoeia and subtitles

- **Onomatopoeia** (`type: "text"`): animated comic lettering over the image (presets `boom`, `pow`, `bang`, `zas`,
  `surprise`, `rumble`; params `text`, `color`, `outline`, `height` in % of the screen, `rot`, `anim`: `pop`, `shake`,
  `grow`, `bounce`, `slide`). Put `at` where the sound comes from. A situation adds its own with `text: true`.
  - If the page already has a drawn sound effect (KABOOM in the art), do **not** add another one on top: the art
    already says it. Use them for sounds the page implies but does not letter, or to emphasise a big moment. Keep it
    rare: at most one or two per page. Write them in the comic's language.
- **Subtitles** (`type: "caption"`): text fixed on screen that does not shake. `style`: `subtitle` (bottom, for
  dialogue or translations), `title` (big in the middle: chapter titles, "Meanwhile…"), `narration` (yellow box top
  left, like a comic caption). Give each line its reading time (about 15 characters per second, never under 1.5 s)
  and do not overlap two subtitles. When exporting, the user can save them as an `.srt` file too.

## Sound

- Library: `list_sounds` (built-in: rain, wind, waves, heartbeat, thunder, hit, punch, whoosh, explosion, knock,
  steps, bell, ding, shot; plus the user's imported and recorded sounds). If the user points to a folder,
  `find_audio_files` then `import_sounds`. Use sound names to choose; you cannot listen to them.
- **Tracks** are shared by all images: one voice track per character (`add_track` bus voice, name it), a music
  track and an atmosphere track (bus ambient), effects in sfx.
- **Voices** (`add_audio` bus voice): place each line at the hold start of the shot that shows its balloon. If
  the user has recorded voices, match them to panels by name or order and ask when unsure.
- **Effects** (`add_audio` bus sfx): on the action (a hit when the punch lands, a door knock, thunder on a lightning
  panel). Short and slightly before or on the frame.
- **Music / ambience** (`add_ambient` from image A to B): loops across images with fades. Music volume 0.4–0.7,
  atmosphere 0.5–0.8. Duck stays on so voices are clear.
- **Audio effects** (`effects_catalog`): reverb hall/cathedral/cave for big places or flashbacks, filter phone or
  radio for voices through devices, pitch monster/robot for creatures, delay mountain for echoes, compressor voice
  to even out recorded voices. Put a track effect for a whole character; a clip effect for one line.
- `sound_report`: peaks near 0 dB with many limiter seconds mean too loud: lower track or group volumes. Fix
  overlapping voices on the same track. Long silences may be fine (dramatic) or need ambience.

## Limits

- You see still frames, not motion: describe moves clearly and ask the user to watch with `play`.
- You cannot hear: rely on names, durations, `sound_report`, and the user's ears.
- Respect what the user already did: before replacing an image's animation or sounds, check `get_timeline`, and
  ask if it looks hand-made. Everything can be undone with `undo`.
- If a tool says AI control is turned off, ask the user to enable it in imgxplorer: Settings → AI control.
