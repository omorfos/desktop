# `rules.yaml`

```yaml
rules:
  - match: { app_id: firefox }
    workspace: 1
  - match: { app_id: "org.gnome.*", title: "*Preferences" }
    maximized: false
    decorations: true
  - match: { app_id: mpv }
    fullscreen: true
  - match: { title: "*Bank*" }
    private: true
  - match: { app_id: org.gnome.Loupe }
    private: blur
  - match: { app_id: "*notes*" }
    above: true
    sticky: true
```

A rule needs `app_id`, `title` or both; `*` stands for any run of
characters. Every matching rule applies, in order. Properties are
`workspace`, `maximized`, `fullscreen`, `decorations`, `private`, and
`shaded` (rolled up into its titlebar), `above` (kept above the windows
that are not) and `sticky` (on every workspace of its screen), which are
what the titlebar's menu changes too. A window without a titlebar is not
rolled up.
Without a `decorations` rule, a window is framed if it asks omorfos to
decorate it.

The others are looked at when a window opens. `private` holds for as
long as the window matches: a window whose title comes to match is
private from then on, and one whose title no longer does is not. A
private window is left blank in any picture a program takes of the
screen, titlebar and all, and so is a window that shares its frame as a
tab. You see it as ever.

`private: blur` has the window blurred in pictures instead, frame and
all, so that someone you share your screen with can see that a window is
there and roughly what is in it, but read nothing of it. A blur hides
less than a blank: colours, large shapes and where things are can still
be made out, so keep `true` (or `blank`) for what nobody else should
know is there, such as a password manager. Where a picture cannot be
blurred, as with `appearance.reduce_transparency`, it is blank; a
picture of the window by itself is blank too; and its menus are left
out of a picture, as a blank window's are. In a frame of several
windows, one to be blank makes all of it blank.
