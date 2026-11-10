---
paths:
  - "src/**"
  - "extension/**"
  - "crates/**"
  - "src-tauri/**"
---

# UI text

Every word the user sees: labels, titles, buttons, hints, notices, toasts, placeholders, and the error messages Rust
sends to the window. Nari wants it short and plain. Flavor text reads as fake and inhuman.

- Write only what the user needs to act or decide: names, states, errors, and facts that change a choice ("Nothing in
  it is moved or deleted" under a not-empty warning).
- No flavor text. Never:
  - descriptions of what a feature does or why it's nice ("Videos, audio, images and saved sites go in folders inside
    it!");
  - taglines, chatty questions as titles ("Where should Scrappy keep everything?"), or cheerful filler ("Ready to
    go", "Empty and ready");
  - roadmap notes ("Arrives in M4") or "coming soon" text: an unbuilt view shows its title only;
  - advice the user doesn't need ("Choose another one" after "Can't write to this folder").
- Titles and buttons are a few words: "Choose a library folder", "Create library", "Library not found".
- A hint only when the control can't say it itself: "Applies after a restart."
- One plain line on what a thing is where the user is about to download, install or allow it, so they know what they
  agree to: "Downloads video and audio from YouTube and other sites" under yt-dlp in the tool list.
- An error says what failed. Add the fix only when it isn't obvious ("Update Scrappy to open it").
