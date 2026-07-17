# Overview - SWE-bench

Source: https://www.swebench.com/SWE-bench/

[
          Skip to content
        ](https://www.swebench.com/SWE-bench/#swe-bench) [

  ![logo](https://www.swebench.com/SWE-bench/assets/swellama.svg)

    ](https://www.swebench.com/SWE-bench/.)
            SWE-bench

              Overview

            Initializing search

[

    SWE-bench

](https://github.com/SWE-bench/SWE-bench) -
      [

  Leaderboard

      ](https://swebench.com)

-
        [

  SWE-bench

        ](https://www.swebench.com/SWE-bench/.)

-
        [

  User Guides

        ](https://www.swebench.com/SWE-bench/guides/quickstart/)

[

  ![logo](https://www.swebench.com/SWE-bench/assets/swellama.svg)

    ](https://www.swebench.com/SWE-bench/.)
    SWE-bench
  [

    SWE-bench

](https://github.com/SWE-bench/SWE-bench) -
      [

    Leaderboard

      ](https://swebench.com)

-

          [

    SWE-bench

            ](https://www.swebench.com/SWE-bench/.)

    SWE-bench

          -
      [

    Installation

      ](https://www.swebench.com/SWE-bench/installation/)

-
      [

    FAQ

      ](https://www.swebench.com/SWE-bench/faq/)

-

    User Guides

    User Guides

          -
      [

    SWE-bench Quickstart

      ](https://www.swebench.com/SWE-bench/guides/quickstart/)

-
      [

    Evaluation

      ](https://www.swebench.com/SWE-bench/guides/evaluation/)

-
      [

    Docker Setup

      ](https://www.swebench.com/SWE-bench/guides/docker_setup/)

-
      [

    Datasets

      ](https://www.swebench.com/SWE-bench/guides/datasets/)

-
      [

    Create RAG Datasets

      ](https://www.swebench.com/SWE-bench/guides/create_rag_datasets/)

-

    Reference

    Reference

          -
      [

    The Harness

      ](https://www.swebench.com/SWE-bench/reference/harness/)

-
      [

    Inference

      ](https://www.swebench.com/SWE-bench/reference/inference/)

-
      [

    Versioning

      ](https://www.swebench.com/SWE-bench/reference/versioning/)

-

    API

    API

          -
      [

    Harness

      ](https://www.swebench.com/SWE-bench/api/harness/)

-
      [

    Inference

      ](https://www.swebench.com/SWE-bench/api/inference/)

-
      [

    Versioning

      ](https://www.swebench.com/SWE-bench/api/versioning/)

      Table of contents
    -
  [

        🔍 All of the Projects

  ](https://www.swebench.com/SWE-bench/#all-of-the-projects)

-
  [

        🏆 Leaderboard

  ](https://www.swebench.com/SWE-bench/#leaderboard)

-
  [

        📋 Overview

  ](https://www.swebench.com/SWE-bench/#overview)

-
  [

        📰 Latest News

  ](https://www.swebench.com/SWE-bench/#latest-news)

-
  [

        🚀 Quick Start

  ](https://www.swebench.com/SWE-bench/#quick-start)

-
  [

        📚 Documentation Structure

  ](https://www.swebench.com/SWE-bench/#documentation-structure)

-
  [

        ⬇️ Available Resources

  ](https://www.swebench.com/SWE-bench/#available-resources)

-
  [

        💫 Contributing

  ](https://www.swebench.com/SWE-bench/#contributing)

-
  [

        ✍️ Citation

  ](https://www.swebench.com/SWE-bench/#citation)

[

    ](https://github.com/SWE-bench/SWE-bench/edit/main/docs/index.md) # SWE-bench

[![Kawi the SWE-Llama](https://www.swebench.com/SWE-bench/assets/figures/swellama_banner_nobg.svg)](https://www.swebench.com/SWE-bench/assets/figures/swellama_banner_nobg.svg) SWE-bench is a benchmark for evaluating large language models on real world software issues collected from GitHub. Given a *codebase* and an *issue*, a language model is tasked with generating a *patch* that resolves the described problem.

## 🔍 All of the Projects

Check out the other projects that are part of the SWE-bench ecosystem!

[
        ![SWE-agent](https://raw.githubusercontent.com/SWE-agent/swe-agent-media/refs/heads/main/media/logos_banners/sweagent_logo_text_right.svg)
    ](https://swe-agent.com) [
        ![SWE-smith](https://raw.githubusercontent.com/SWE-agent/swe-agent-media/refs/heads/main/media/logos_banners/swesmith_logo_text_right.svg)
    ](https://swesmith.com) [
        ![SWE-rex](https://raw.githubusercontent.com/SWE-agent/swe-agent-media/refs/heads/main/media/logos_banners/swerex_button_text_right.svg)
    ](https://swe-rex.com) [
        ![CodeClash](https://raw.githubusercontent.com/SWE-agent/swe-agent-media/refs/heads/main/media/logos_banners/codeclash_logo_text_right.svg)
    ](https://codeclash.ai) [
        ![SWE-bench CLI](https://raw.githubusercontent.com/SWE-agent/swe-agent-media/refs/heads/main/media/logos_banners/sbcli_logo_text_right.svg)
    ](https://swebench.com/sb-cli) [
        ![mini-swe](https://www.swebench.com/SWE-bench/assets/icons/mini-swe-agent-banner.svg)
    ](https://mini-swe-agent.com) ## 🏆 Leaderboard

You can find the full leaderboard at [swebench.com](https://swebench.com)!

## 📋 Overview

SWE-bench provides:

- ✅ **Real-world GitHub issues** - Evaluate LLMs on actual software engineering tasks
- ✅ **Reproducible evaluation** - Docker-based evaluation harness for consistent results
- ✅ **Multiple datasets** - SWE-bench, SWE-bench Lite, SWE-bench Verified, and SWE-bench Multimodal

## 📰 Latest News

- **[Jan. 13, 2025]**: SWE-bench Multimodal integration with private test split evaluation
- **[Jan. 11, 2025]**: Cloud-based evaluations [via Modal](https://www.swebench.com/SWE-bench/guides/evaluation/)
- **[Aug. 13, 2024]**: SWE-bench Verified release with 500 engineer-confirmed solvable problems
- **[Jun. 27, 2024]**: Fully containerized evaluation harness using Docker
- **[Apr. 2, 2024]**: SWE-agent release with state-of-the-art results
- **[Jan. 16, 2024]**: SWE-bench accepted to ICLR 2024 as an oral presentation

## 🚀 Quick Start

```
# Access SWE-bench via Hugging Face
from datasets import load_dataset
swebench = load_dataset('princeton-nlp/SWE-bench', split='test')
```

```
# Setup with Docker
git clone [email protected]:princeton-nlp/SWE-bench.git
cd SWE-bench
pip install -e .
```

## 📚 Documentation Structure

- **[Installation](https://www.swebench.com/SWE-bench/installation/)** - Setup instructions for local and cloud environments
- **Guides**
- [Quickstart](https://www.swebench.com/SWE-bench/guides/quickstart/) - Get started with SWE-bench
- [Evaluation](https://www.swebench.com/SWE-bench/guides/evaluation/) - How to evaluate models on SWE-bench
- [Docker Setup](https://www.swebench.com/SWE-bench/guides/docker_setup/) - Configure Docker for SWE-bench
- [Datasets](https://www.swebench.com/SWE-bench/guides/datasets/) - Available datasets and how to use them
- [Create RAG Datasets](https://www.swebench.com/SWE-bench/guides/create_rag_datasets/) - Build your own retrieval datasets
- **Reference**
- [Harness API](https://www.swebench.com/SWE-bench/reference/harness/) - Documentation for the evaluation harness
- [Inference API](https://www.swebench.com/SWE-bench/reference/inference/) - Documentation for model inference
- [Versioning](https://www.swebench.com/SWE-bench/reference/versioning/) - Documentation for versioning
- **[FAQ](https://www.swebench.com/SWE-bench/faq/)** - Frequently asked questions

## ⬇️ Available Resources

| Datasets               | Models                 | RAG                      |
| ---------------------- | ---------------------- | ------------------------ |
| 💿 SWE-bench            | 🦙 SWE-Llama 13b        | 🤗 "Oracle" Retrieval     |
| 💿 SWE-bench Lite       | 🦙 SWE-Llama 13b (PEFT) | 🤗 BM25 Retrieval 13K     |
| 💿 SWE-bench Verified   | 🦙 SWE-Llama 7b         | 🤗 BM25 Retrieval 27K     |
| 💿 SWE-bench Multimodal | 🦙 SWE-Llama 7b (PEFT)  | 🤗 BM25 Retrieval 40K/50K |

## 💫 Contributing

We welcome contributions from the NLP, Machine Learning, and Software Engineering communities! Please check our [contributing guidelines](https://github.com/princeton-nlp/SWE-bench/blob/main/docs/README.md) for details.

## ✍️ Citation

```
@inproceedings{
    jimenez2024swebench,
    title={{SWE}-bench: Can Language Models Resolve Real-world Github Issues?},
    author={Carlos E Jimenez and John Yang and Alexander Wettig and Shunyu Yao and Kexin Pei and Ofir Press and Karthik R Narasimhan},
    booktitle={The Twelfth International Conference on Learning Representations},
    year={2024},
    url={https://openreview.net/forum?id=VTF8yNQM66}
}

@inproceedings{
    yang2024swebenchmultimodal,
    title={{SWE}-bench Multimodal: Do AI Systems Generalize to Visual Software Domains?},
    author={John Yang and Carlos E. Jimenez and Alex L. Zhang and Kilian Lieret and Joyce Yang and Xindi Wu and Ori Press and Niklas Muennighoff and Gabriel Synnaeve and Karthik R. Narasimhan and Diyi Yang and Sida I. Wang and Ofir Press},
    booktitle={The Thirteenth International Conference on Learning Representations},
    year={2025},
    url={https://openreview.net/forum?id=riTiq3i21b}
}
```

[

                Next

                Installation

          ](https://www.swebench.com/SWE-bench/installation/)

    Made with
    [
      Material for MkDocs
    ](https://squidfunk.github.io/mkdocs-material/)
