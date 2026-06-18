Use `image_generate` for text-to-image or reference-guided image generation.
Create high-quality, visually consistent images for the website or visual work by using the same set of prompts to maintain a unified aesthetic in multiple image_generate.
Inputs can be CLI text or JSON. Supply a positive `prompt`; optionally supply `negative_prompt`, `references`, and common image parameters: `width`, `height`, `size`, `aspect_ratio`, `quality`, `n`, `seed`, and `output_format`.
Always make sure generated media does not look like AI slop: it must have a strong visual identity, avoid noise, and feel intentionally art-directed.
When multiple images are needed, generate them in one `command_run` batch with separate `image_generate` commands in the same step, using matching style language, dimensions, quality, and output directories so the results belong to one design system.

Example: generate two related images with two `image_generate` commands in one batch.

```json
{
  "requests": {
    "commands": [
      {
        "command": "image_generate",
        "step": 1,
        "command_line": "{\"prompt\":\"full-bleed editorial hero image for a minimalist furniture website, warm natural light, quiet premium design system, generous negative space\",\"negative_prompt\":\"clutter, noisy background, distorted text, low quality\",\"aspect_ratio\":\"16:9\",\"quality\":\"high\",\"output_dir\":\"media/furniture-hero\"}"
      },
      {
        "command": "image_generate",
        "step": 1,
        "command_line": "{\"prompt\":\"matching product detail image for the same minimalist furniture website, close crop of natural material texture, warm natural light, quiet premium design system\",\"negative_prompt\":\"clutter, noisy background, distorted text, low quality\",\"aspect_ratio\":\"4:3\",\"quality\":\"high\",\"output_dir\":\"media/furniture-detail\"}"
      }
    ]
  }
}
```
