Use `generate_media` for creating new media assets.

Input is CLI-style. Use `image` or `speech` as the first word, quote long prompts/text, and pass options as flags.

For images, pass `image` with a strong visual prompt. Optional image fields are `--reference`, `--width`, `--height`, `--size`, `--aspect-ratio`, `--quality`, `--n`, `--seed`, `--format`, and `--output-dir`.

Image prompts must be positive-only. Always describe and verify the time period settings. For character image prompts, you must describe the distinct character's style, facial features, body type/build, hairstyle, posture, and visual identity in detail.

Describe only the elements that appear in the media, and do not include any information about the media’s intended use. Do not include the name of any character, any text that describe the object, or words such as "magazine cover", poser in the prompt.

Image prompts must avoid AI slop, safe stock-like defaults, and CGI-heavy fantasy/game visuals by choosing concrete positive art direction such as stylized, atmospheric, cinematic, editorial...

If an image may need background removal later, prompt for a single isolated subject on a plain solid background with a color that strongly contrasts the subject and is unrelated to the final theme palette, with crisp visible subject edges.

Generate multiple distinct media assets with multiple separate `generate_media` calls in small batches. Do not ask one call to create several different assets, scenes, characters, icons, backgrounds, or deliverables at once. 

Keep each image call focused on one clear asset and use repeated prompt + modification calls for asset sets so each output can preserve the intended style direction and quality.

For speech audio, pass `speech` with only semantic voice controls: text, `--text-language`, `--role`, `--tone`, and optionally `--custom-tone-description` and `--custom-voice-description`. Do not choose or mention providers; provider fallback is configured by the system.

Speech enums:
- `text_language`: `zh_cn`, `en_us`, `ja_jp`, `ko_kr`, `es_es`, `fr_fr`
- `role`: `female_gentle`, `female_bright`, `female_confident`, `female_young`, `male_calm`, `male_warm`, `male_deep`, `male_energetic`
- `tone`: `neutral`, `calm`, `cheerful`, `serious`, `sad`, `whisper`

Example image command:

```text
image "full-bleed editorial headshot portrait of a young athletic Asian woman with a short square face, almond-shaped eyes,  and a sleek high ponytail, robust build, poised upright posture with relaxed shoulders, wearing fitted matte yoga leggings and a minimal fitted top, grey blank background, atmospheric editorial photography." --size 1536x864 --quality high --output-dir media/furniture-hero
```

Example speech command:

```text
speech "Welcome back. Today, we'll continue refining the product experience to make it feel more natural." --text-language en_us --role female_gentle --tone calm --custom-tone-description "A soft, intimate voice-over, as if speaking close to the listener's ear"
```
