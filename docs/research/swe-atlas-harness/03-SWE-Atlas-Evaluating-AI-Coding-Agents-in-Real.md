# SWE Atlas: Evaluating AI Coding Agents in Real Codebases | Scale AI

Source: https://scale.com/blog/swe-atlas

Scale partners with Mayo Clinic to develop reliable AI for healthcare[Read the Full Story](https://scale.com/blog/mayo-clinic-scale) [](https://scale.com/)ProductsSolutionsResearchResources[Log in](https://dashboard.scale.com/login)[Book demo](https://scale.com/demo)[Book demo](https://scale.com/demo)[← Blog](https://scale.com/blog)[Research](https://scale.com/blog/category/research)# Can Coding Agents Become Engineers? We’re Finding Out.

By [The Scale Research Team](https://scale.com/blog/author/the-scale-research-team) & [Matthew Siegel](https://scale.com/blog/author/matthew-siegel)·March 4, 2026·4 min read Copy Link![blog header image with "SEAL" and "SWE Atlas" at the top.](https://scale.com/_next/image?url=https%3A%2F%2Fcdn.sanity.io%2Fimages%2F50zba0eo%2Fproduction%2Fe4dc579bf506845df622c274132edb4e9625cf2e-1200x675.png%3Fauto%3Dformat%26q%3D80&w=1920&q=75)Executives at major tech companies have claimed that top software engineers are no longer writing code. As LLMs and coding agents take on that work, evaluation must evolve to view them more like junior engineers: by how they investigate a system, gather evidence, and explain what they’re observing.

Today, we’re launching [SWE Atlas](https://scale.com/leaderboard/sweatlas-qna), the first evaluation suite of its kind, to do just that.

SWE Atlas is composed of three separate evaluations with leaderboards that assess how agents understand, validate, and improve real software systems inside real repositories. The evaluations include:

- **Codebase QnA** - Understand complex codebases through runtime analysis and multi-file reasoning
- **Test Writing** - Write meaningful tests that exercise real functionality to increase code coverage
- **Refactoring** - Restructure code to improve readability & maintainability while preserving behavior

Of these evaluations, Codebase QnA is available today, with Test Writing and Refactoring available soon.

## Complementing SWE-Bench Pro

SWE Atlas builds on the foundation of Scale’s SWE-Bench Pro (recommended by [OpenAI](https://openai.com/index/why-we-no-longer-evaluate-swe-bench-verified/) for frontier releases), drawing from the same production repos and environments while expanding evaluation to investigative and maintenance workflows.

![](https://scale.com/_next/image?url=https%3A%2F%2Fcdn.sanity.io%2Fimages%2F50zba0eo%2Fproduction%2F73893c3ed71edeced19be6cbcaf70d0805a26c4f-1324x726.png&w=1920&q=80)SWE Atlas extends evaluation beyond change correctness to the investigative and maintenance workflows that surround real software development.

## Evaluating Agents Inside Real Systems

SWE Atlas runs agents inside reproducible environments built from real software repos. Agents can inspect code, run commands, and execute the system, allowing evaluation to focus on how they investigate behavior, validate assumptions, and produce explanations grounded in runtime evidence. This approach pushes measurement toward interaction with a working codebase rather than evaluation based solely on final outputs.

## What Codebase QnA Tasks Look Like

Codebase QnA tasks reflect the kinds of questions engineers ask when investigating real systems. For example, an onboarding QnA task looks like a question a new engineer would ask while onboarding onto a codebase: “When I run kitten @ ls from another terminal, how does that command reach the running kitty instance and get processed?” Answering that requires tracing Unix socket communication, IPC framing, and command dispatch across both C and Python, then validating behavior by running the system.

QnA tasks span multiple investigation types, including architecture and system design, root-cause analysis, onboarding, security reasoning, and API or library integration, for example: explaining unexpected runtime behavior, understanding unfamiliar systems, or tracing security boundaries.

The dataset is designed to mirror real engineering conditions: net-new tasks authored by professional engineers and technical experts inside open-source repositories drawn from the same production codebases used in SWE-Bench Pro, including systems like terminal emulators, mail servers, and object storage platforms, spanning multiple architectures and languages (Go, Python, C, TypeScript). Tasks pass through multi-stage review and are evaluated with structured rubrics that score whether an agent’s explanation reflects how the system actually works.

Codebase QnA is the first released benchmark, with test writing and refactoring expanding the evaluation surface over time.

## How We Measure

SWE Atlas runs agents inside reproducible, sandboxed environments where they can use standard developer tools to inspect code, run commands, and execute the system itself, using the SWE-Agent scaffold to standardize interaction and evaluation. We also evaluated some models with Claude Code Harness and Codex CLI using the model’s native scaffolds. Scoring combines programmatic checks with expert-defined rubrics and focuses on how agents interact with the running system. The primary metric is Task Resolve Rate: the share of tasks where every rubric item is satisfied.

Full results and ongoing updates are available on the [leaderboard](https://scale.com/leaderboard/sweatlas-qna).

## From Investigation to Collaboration

The next phase of AI coding reflects that reality: agents that can modify software while reasoning about complex environments. SWE Atlas begins to map the investigative, validation, and maintenance workflows that define real software engineering. As the benchmark expands and the ecosystem participates, the definition of a capable coding agent will move from code generator to system collaborator.

Keep an eye on the Scale blog for updates on our next two SWE Atlas benchmarks: Test Writing and Refactoring.

## Ready to break through your data bottleneck?

Scale's team will match your project to the right experts, fast.

[Talk to our experts](https://scale.com/demo)![](https://scale.com/static/img/reskin/prefooter.jpg?dpl=dpl_7ddP2apUGzDdxsk7W9MxamEtJhuA)Products

[Scale data engine](https://scale.com/data-engine)[Scale GenAI Platform](https://scale.com/genai-platform)[Scale Donovan](https://scale.com/donovan)Solutions

[Enterprise](https://scale.com/enterprise/agentic-solutions)[Insurance](https://scale.com/enterprise/insurance)[Healthcare](https://scale.com/enterprise/healthcare)[US Public Sector](https://scale.com/public-sector)[Global Public Sector](https://scale.com/global-public-sector)Company

[About](https://scale.com/about)[Careers](https://scale.com/careers)[Security](https://scale.com/security)[Terms](https://scale.com/legal/terms)[Privacy](https://scale.com/legal/privacy)[Modern Slavery Statement](https://scale.com/legal/modern-slavery-statement)Resources

[Blog](https://scale.com/blog)[Contact Us](https://scale.com/demo)[Events](https://scale.com/events)[Documentation](https://scale.com/docs)[Data Partnerships](https://scale.com/data-partnership)[Brand Guidelines](https://brand.scale.com/)Guides

[Data Labeling](https://scale.com/guides/data-labeling-annotation-guide)[ML Model Training](https://scale.com/guides/model-training-building)[Diffusion Models](https://scale.com/guides/diffusion-models-guide)[Guide to AI for eCommerce](https://scale.com/guides/ai-for-ecommerce)[Computer Vision Applications](https://scale.com/guides/computer-vision)[Large Language Models](https://scale.com/guides/large-language-models)# Reliable AI for the world’s most important decisions

[![](https://cdn.sanity.io/images/50zba0eo/production/777236d99c2ab67069cdd0a29117dd5a47366ec7-16x16.svg?auto=format&w=1920&width=1920)](https://www.linkedin.com/company/scaleai)[![](https://cdn.sanity.io/images/50zba0eo/production/6ce2239ab36209947167f2ccac45d8eebfe7fe9f-16x16.svg?auto=format&w=1920&width=1920)](https://x.com/scale_ai)Manage your cookie preferences

Copyright © 2026 Scale AI, Inc. All rights reserved

[Terms of Use](https://scale.com/legal/terms) & [Privacy Policy](https://scale.com/legal/privacy)

## Media links

- <https://scale.com/static/global/favicon/apple-touch-icon.png>
- <https://scale.com/static/img/reskin/prefooter.jpg?dpl=dpl_7ddP2apUGzDdxsk7W9MxamEtJhuA>
- <https://cdn.sanity.io/images/50zba0eo/production/e4dc579bf506845df622c274132edb4e9625cf2e-1200x675.png?auto=format&q=80>
- <https://cdn.sanity.io/images/50zba0eo/production/2defa595321deaa642860bb3a994f5e80271def1-1456x816.png?auto=format>
- <https://cdn.sanity.io/images/50zba0eo/production/f443457bd9d54803b854f4cf102c620ecf13b643-1921x1079.png?auto=format>
- <https://cdn.sanity.io/files/50zba0eo/production/68a8abe35c8b29a4f292e634bc11fa1e9f59c5a9.mp4>
- <https://cdn.sanity.io/images/50zba0eo/production/d4abdaada1957376bd2d98b41ecf448be772f40b-1080x1080.png>
- <https://cdn.sanity.io/files/50zba0eo/production/4332b6ccc19e48ac472e3dae267a2b29eb2ff009.webm>
- <https://cdn.sanity.io/images/50zba0eo/production/6442a1568a3ef5a2949da53e6ac29b1ad064c4b8-1080x1080.png>
- <https://cdn.sanity.io/images/50zba0eo/production/73893c3ed71edeced19be6cbcaf70d0805a26c4f-1324x726.png>
- <https://cdn.sanity.io/images/50zba0eo/production/e4dc579bf506845df622c274132edb4e9625cf2e-1200x675.png?auto=format>
