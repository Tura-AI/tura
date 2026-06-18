Use `generate_media` for creating new media assets.

For images, pass `media_type: "image"` with a strong visual `prompt`. Optional image fields are `negative_prompt`, `references`, `width`, `height`, `size`, `aspect_ratio`, `quality`, `n`, `seed`, `output_format`, and `output_dir`.

For speech audio, pass `media_type: "speech"` with only semantic voice controls: `text`, `text_language`, `role`, `tone`, and optionally `custom_tone_description` and `custom_voice_description`. Do not choose or mention providers; provider fallback is configured by the system.

Speech enums:
- `text_language`: `zh_cn`, `en_us`, `ja_jp`, `ko_kr`, `es_es`, `fr_fr`
- `role`: `female_gentle`, `female_bright`, `female_confident`, `female_young`, `male_calm`, `male_warm`, `male_deep`, `male_energetic`
- `tone`: `neutral`, `calm`, `cheerful`, `serious`, `sad`, `whisper`

Example image command:

```json
{
  "command": "generate_media",
  "command_line": "{\"media_type\":\"image\",\"prompt\":\"full-bleed editorial hero image for a minimalist furniture website, warm natural light, quiet premium design system, generous negative space\",\"negative_prompt\":\"clutter, noisy background, distorted text, low quality\",\"aspect_ratio\":\"16:9\",\"quality\":\"high\",\"output_dir\":\"media/furniture-hero\"}"
}
```

Example speech command:

```json
{
  "command": "generate_media",
  "command_line": "{\"media_type\":\"speech\",\"text\":\"欢迎回来，今天我们继续把产品体验打磨得更自然。\",\"text_language\":\"zh_cn\",\"role\":\"female_gentle\",\"tone\":\"calm\",\"custom_tone_description\":\"像贴近耳边的轻声旁白\"}"
}
```
