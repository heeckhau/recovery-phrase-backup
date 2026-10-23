# Talk slides

A short introduction to Recovery Phrase Backup: the problem with one copy of a
recovery phrase, how splitting it into SLIP-39 shares works, why this tool
exists and how far you have to trust it, followed by a live demo.

`slides.md` is the source, in [Marp](https://marp.app) Markdown, styled like
the website. `build.sh` turns it into `slides.html`, one file with everything
inlined that opens offline in any browser:

```sh
talk/build.sh    # needs Node.js (for npx) and Python 3
```

Open `slides.html` and use the arrow keys: `F` for full screen, `P` for the
presenter view. The share card shown is from the demo, made from its published
sample phrase.
