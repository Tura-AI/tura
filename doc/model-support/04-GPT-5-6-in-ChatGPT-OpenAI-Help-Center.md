# GPT-5.6 in ChatGPT | OpenAI Help Center

Source: https://help.openai.com/en/articles/20001325-a-preview-of-gpt-56-sol-terra-and-luna

[![OpenAI](https://help.openai.com/logo.png)](https://help.openai.com/en)1. [All Collections](https://help.openai.com/en)
2. [ChatGPT](https://help.openai.com/en/collections/3742473-chatgpt)
3. GPT-5.6 in ChatGPT

# GPT-5.6 in ChatGPT

Learn how GPT-5.6 Sol works in ChatGPT, which options are available by plan, and how usage limits and availability work.

Updated: 16 hours ago*GPT‑5.6 is gradually rolling out to eligible ChatGPT plans. If you do not see GPT‑5.6 Sol in the ChatGPT model picker, it may not be available for your account yet.*

*To access GPT‑5.6 in Codex, use one of these minimum versions:*

- *ChatGPT desktop app (Codex mode): 26.707.30751*

- *Codex CLI: 0.144.0*

# Overview

GPT-5.6 Sol is designed for complex work across coding, knowledge work and research, cybersecurity, science, computer use, and design. GPT-5.6 Sol Pro is the highest-capability GPT-5.6 option for difficult tasks and longer-running workflows.

GPT-5.5 Instant remains the default for fast, everyday responses. GPT-5.6 Sol now powers the **Medium**, **High**, and **Extra High** reasoning options on eligible plans, while GPT-5.6 Sol **Pro** powers Pro.

# How GPT-5.6 works in ChatGPT

On eligible paid plans, ChatGPT can automatically use more reasoning for complex requests. In the model picker, this appears as **Instant** switching to **Medium**. Select **Configure** in the model picker to turn automatic switching on or off.

You can also choose a reasoning level directly from the model picker. The options available depend on your plan and, for managed workspaces, workspace access settings.

# Choose a speed and reasoning level

- **Instant** — Fast responses for everyday questions, powered by GPT-5.5 Instant.

- **Medium** — Standard reasoning with GPT-5.6 Sol.

- **High** — Extended reasoning with GPT-5.6 Sol.

- **Extra High** — The highest reasoning effort available with GPT-5.6 Sol.

- **Pro** — GPT-5.6 Sol Pro for difficult tasks and longer-running workflows.

# Availability

## Availability by ChatGPT plan

The table below shows the GPT-5.6 options available in standard ChatGPT conversations. GPT-5.5 Instant is not shown because it remains the default model.

| ChatGPT plan | Medium and High | Extra High   | Pro          |
| ------------ | --------------- | ------------ | ------------ |
| Plus         | Included        | Not included | Not included |
| Pro          | Included        | Included     | Included     |
| Business     | Included        | Included     | Included     |
| Enterprise   | Included        | Included     | Included     |
| Free and Go  | Not included    | Not included | Not included |

Logged-out users do not have access to GPT-5.6 Sol. In Business and Enterprise workspaces, admins may control which models workspace members can use.

## Availability in Work, Codex, and the OpenAI API

GPT-5.6 Terra and GPT-5.6 Luna are not selectable in standard ChatGPT conversations. Their availability in other products is shown below.

| Product         | GPT-5.6 availability                                                                |
| --------------- | ----------------------------------------------------------------------------------- |
| Work in ChatGPT | Sol, Terra, and Luna for Plus, Pro, Business, and Enterprise                        |
| Codex           | Terra for Free and Go; Sol, Terra, and Luna for Plus, Pro, Business, and Enterprise |
| OpenAI API      | Sol, Terra, and Luna                                                                |

Terra provides a balance of capability, speed, and cost for everyday work. Luna is the fastest and lowest-cost model in the GPT-5.6 family.

## Regional availability

GPT-5.6 does not have a separate country list. ChatGPT access follows the [ChatGPT supported countries and territories](https://help.openai.com/en/articles/7947663-chatgpt-supported-countries), and OpenAI API access follows the [OpenAI API supported countries and territories](https://help.openai.com/en/articles/5347006-openai-api-supported-countries-and-territories).

Eligible users in supported European locations, including the EEA, Switzerland, and the United Kingdom, and in the UAE can use GPT-5.6 when it is available for their plan and product.

Country availability and inference residency are separate. GPT-5.6 is not currently supported for workloads configured to use UAE inference residency. This does not prevent eligible users located in the UAE from using GPT-5.6 without that residency configuration.

For more information, see [Data residency and inference residency for ChatGPT](https://help.openai.com/en/articles/9903489-data-residency-and-inference-residency-for-chatgpt) and [API data residency](https://platform.openai.com/docs/guides/your-data#data-residency).

# Usage limits

GPT-5.6 uses the existing ChatGPT limits for automatic reasoning, manually selected reasoning, and **Pro**. Limits depend on your plan and, for managed workspaces, workspace settings.

Automatically switching does not count toward the usage allowance for manually selected reasoning.

## Instant limits

Free-tier users have limited GPT-5.5 Instant access within a 5-hour window. These limits can be dynamic and may vary based on factors that include market, system conditions, abuse-prevention guardrails, and individual usage.

ChatGPT Plus and ChatGPT Go users can send up to 160 messages with GPT-5.5 Instant every 3 hours. After you reach this limit, chats switch to GPT-5.5 Instant mini until the limit resets.

## Reasoning usage and fallbacks

Manually choosing **Medium**, **High**, or **Extra High** uses GPT-5.6 Sol. If you reach a GPT-5.6 reasoning limit, ChatGPT may continue with GPT-5.4 Thinking mini.

If you are on a Go plan, select **+** in the chat entry box, then select **Thinking**. Go users can send up to 10 messages every 5 hours after enabling **Thinking**. This option does not use GPT-5.6 Sol.

ChatGPT displays when an allowance resets when that information is available. You can use another available model or wait until the displayed reset time.

## Managed workspaces and the ChatGPT Pro plan

For current managed-workspace limits and credit rates, see [ChatGPT Business - Models and Limits](https://help.openai.com/en/articles/12003714-chatgpt-business-models-limits), [ChatGPT Enterprise and Edu - Models and Limits](https://help.openai.com/en/articles/11165333-chatgpt-enterprise-and-edu-models-limits), and the [ChatGPT Rate Card](https://help.openai.com/en/articles/11481834-chatgpt-rate-card-business-enterpriseedu).

On the ChatGPT Pro plan, some models have separate usage allowances. When you reach a model's allowance, that model may be temporarily unavailable until the allowance resets. Reaching an allowance does not by itself mean that your account was restricted.

For more information, see [About ChatGPT Pro tiers](https://help.openai.com/en/articles/9793128).

Usage must follow the [Terms of Use](https://openai.com/policies/terms-of-use/). OpenAI uses safeguards to help prevent misuse, which may occasionally result in a temporary usage restriction. ChatGPT notifies you if this happens.

# Safeguards

GPT-5.6 includes protections trained into the model, along with real-time checks and monitoring. Some higher-risk biological and cybersecurity requests may be refused or require additional checks. These safeguards can occasionally affect legitimate requests.

# FAQ

## Why do I not see GPT-5.6 Sol?

Confirm that GPT-5.6 Sol is included with your plan and that you are signed in to the correct account. If you use a managed workspace, ask an admin whether the model is available for your workspace or role.

GPT-5.6 is rolling out gradually, so it may not be available for every eligible account yet.

## Does GPT-5.6 replace GPT-5.5 Instant?

No. GPT-5.5 Instant remains the default for fast, everyday responses. GPT-5.6 Sol powers the reasoning options on eligible plans, and GPT-5.6 Sol **Pro** powers Pro where available.

## Can I select GPT-5.6 Terra or GPT-5.6 Luna in ChatGPT?

Terra and Luna are not selectable in standard ChatGPT conversations. Depending on your plan, they are available in Work and Codex. Developers can also use them through the OpenAI API.

## Does GPT-5.6 change my ChatGPT usage limits?

GPT-5.6 uses the existing ChatGPT limits for automatic reasoning, manually selected reasoning, and Pro. For managed workspaces, see the plan-specific Models and Limits article or the [ChatGPT Rate Card](https://help.openai.com/en/articles/11481834-chatgpt-rate-card-business-enterpriseedu) for current limits and credit rates.

## Was this article helpful?

Submit
![OpenAI logo](https://help.openai.com/favicon.png)[ChatGPT](https://chatgpt.com)[API](https://platform.openai.com/docs/)[Service Status](https://status.openai.com)Cookie Preferences**We use cookies** and similar technologies to deliver, maintain, improve our services and for security purposes.

Check our [Cookie Policy](https://openai.com/policies/cookie-policy) for details. Click 'Accept all' to let OpenAI and partners use cookies for these purposes. Click 'Reject all' to say no to cookies, except those that are strictly necessary. You can change your cookie settings at any time by clicking the cookie preferences link at the bottom of the page.

Reject AllAccept All

## Media links

- <https://help.openai.com/logo.png>
- <https://help.openai.com/favicon.png>
- <https://help.openai.com/apple-touch-icon.png>
- <https://help.openai.com/og-image.png>
