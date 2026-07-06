# Rich text

Tura uses a restricted HTML-flavored rich text protocol for normal interactive
messages and a separate plain terminal style for CLI sessions.

The full reference is [docs/core/html-rich-text.md](../../docs/core/html-rich-text.md).

## Protocol examples

```html
Use <b>bold</b>, <i>italic</i>, <code>inline code</code>,
<a href='https://example.com'>links</a>, and
<pre><code class='language-python'>print('hello')</code></pre>.
```

```text
[MEDIA:relative/or/absolute/path.png:MEDIA]
[EMOJI:react:👍:EMOJI]
[EMOJI:sticker:😂:EMOJI]
```

## Related

- [Personas](personas.md)
- [Graphic user interface](../architecture/graphic-user-interface.md)
- [Terminal user interface](../architecture/terminal-user-interface.md)
