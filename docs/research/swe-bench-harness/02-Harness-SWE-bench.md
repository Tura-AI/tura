# Harness - SWE-bench

Source: https://www.swebench.com/SWE-bench/api/harness/

[
          Skip to content
        ](https://www.swebench.com/SWE-bench/api/harness/#harness-api) [

  ![logo](https://www.swebench.com/SWE-bench/api/harness/../../assets/swellama.svg)

    ](https://www.swebench.com/SWE-bench/api/harness/../..)
            SWE-bench

              Harness

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

        ](https://www.swebench.com/SWE-bench/api/harness/../..)

-
        [

  User Guides

        ](https://www.swebench.com/SWE-bench/api/harness/../../guides/quickstart/)

[

  ![logo](https://www.swebench.com/SWE-bench/api/harness/../../assets/swellama.svg)

    ](https://www.swebench.com/SWE-bench/api/harness/../..)
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

            ](https://www.swebench.com/SWE-bench/api/harness/../..)

    SWE-bench

          -
      [

    Installation

      ](https://www.swebench.com/SWE-bench/api/harness/../../installation/)

-
      [

    FAQ

      ](https://www.swebench.com/SWE-bench/api/harness/../../faq/)

-

    User Guides

    User Guides

          -
      [

    SWE-bench Quickstart

      ](https://www.swebench.com/SWE-bench/api/harness/../../guides/quickstart/)

-
      [

    Evaluation

      ](https://www.swebench.com/SWE-bench/api/harness/../../guides/evaluation/)

-
      [

    Docker Setup

      ](https://www.swebench.com/SWE-bench/api/harness/../../guides/docker_setup/)

-
      [

    Datasets

      ](https://www.swebench.com/SWE-bench/api/harness/../../guides/datasets/)

-
      [

    Create RAG Datasets

      ](https://www.swebench.com/SWE-bench/api/harness/../../guides/create_rag_datasets/)

-

    Reference

    Reference

          -
      [

    The Harness

      ](https://www.swebench.com/SWE-bench/api/harness/../../reference/harness/)

-
      [

    Inference

      ](https://www.swebench.com/SWE-bench/api/harness/../../reference/inference/)

-
      [

    Versioning

      ](https://www.swebench.com/SWE-bench/api/harness/../../reference/versioning/)

-

    API

    API

          -

    Harness

      [

    Harness

      ](https://www.swebench.com/SWE-bench/api/harness/./)

      Table of contents
    -
  [
     harness

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness)

    -
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.__all__)

-
  [
     constants

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants)

    -
  [
     SPECS_REDIS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_REDIS)

-
  [
     SPECS_JQ

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_JQ)

-
  [
     SPECS_JSON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_JSON)

-
  [
     SPECS_MICROPYTHON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_MICROPYTHON)

-
  [
     SPECS_VALKEY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_VALKEY)

-
  [
     SPECS_FMT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FMT)

-
  [
     MAP_REPO_VERSION_TO_SPECS_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_C)

-
  [
     MAP_REPO_TO_INSTALL_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_C)

-
  [
     SPECS_CADDY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_CADDY)

-
  [
     SPECS_TERRAFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_TERRAFORM)

-
  [
     SPECS_PROMETHEUS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PROMETHEUS)

-
  [
     SPECS_HUGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_HUGO)

-
  [
     SPECS_GIN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_GIN)

-
  [
     MAP_REPO_VERSION_TO_SPECS_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_GO)

-
  [
     MAP_REPO_TO_INSTALL_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_GO)

-
  [
     SPECS_GSON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_GSON)

-
  [
     SPECS_DRUID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_DRUID)

-
  [
     SPECS_JAVAPARSER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_JAVAPARSER)

-
  [
     SPECS_LOMBOK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_LOMBOK)

-
  [
     SPECS_LUCENE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_LUCENE)

-
  [
     SPECS_RXJAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_RXJAVA)

-
  [
     MAP_REPO_VERSION_TO_SPECS_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_JAVA)

-
  [
     MAP_REPO_TO_INSTALL_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_JAVA)

-
  [
     TEST_XVFB_PREFIX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_XVFB_PREFIX)

-
  [
     XVFB_DEPS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.XVFB_DEPS)

-
  [
     X11_DEPS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.X11_DEPS)

-
  [
     SPECS_CALYPSO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_CALYPSO)

-
  [
     TEST_CHART_JS_TEMPLATE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_CHART_JS_TEMPLATE)

-
  [
     SPECS_CHART_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_CHART_JS)

-
  [
     SPECS_MARKED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_MARKED)

-
  [
     SPECS_P5_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_P5_JS)

-
  [
     SPECS_REACT_PDF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_REACT_PDF)

-
  [
     JEST_JSON_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.JEST_JSON_JQ_TRANSFORM)

-
  [
     SPECS_BABEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_BABEL)

-
  [
     SPECS_VUEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_VUEJS)

-
  [
     SPECS_DOCUSAURUS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_DOCUSAURUS)

-
  [
     SPECS_IMMUTABLEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_IMMUTABLEJS)

-
  [
     SPECS_THREEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_THREEJS)

-
  [
     SPECS_PREACT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PREACT)

-
  [
     SPECS_AXIOS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_AXIOS)

-
  [
     MAP_REPO_VERSION_TO_SPECS_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_JS)

-
  [
     MAP_REPO_TO_INSTALL_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_JS)

-
  [
     SPECS_PHPSPREADSHEET

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PHPSPREADSHEET)

-
  [
     SPECS_LARAVEL_FRAMEWORK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_LARAVEL_FRAMEWORK)

-
  [
     SPECS_PHP_CS_FIXER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PHP_CS_FIXER)

-
  [
     SPECS_CARBON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_CARBON)

-
  [
     MAP_REPO_VERSION_TO_SPECS_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_PHP)

-
  [
     MAP_REPO_TO_INSTALL_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_PHP)

-
  [
     TEST_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_PYTEST)

-
  [
     TEST_PYTEST_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_PYTEST_VERBOSE)

-
  [
     TEST_ASTROPY_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_ASTROPY_PYTEST)

-
  [
     TEST_DJANGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_DJANGO)

-
  [
     TEST_DJANGO_NO_PARALLEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_DJANGO_NO_PARALLEL)

-
  [
     TEST_SEABORN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SEABORN)

-
  [
     TEST_SEABORN_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SEABORN_VERBOSE)

-
  [
     TEST_SPHINX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SPHINX)

-
  [
     TEST_SYMPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SYMPY)

-
  [
     TEST_SYMPY_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SYMPY_VERBOSE)

-
  [
     SPECS_SKLEARN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SKLEARN)

-
  [
     SPECS_FLASK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FLASK)

-
  [
     SPECS_DJANGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_DJANGO)

-
  [
     SPECS_REQUESTS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_REQUESTS)

-
  [
     SPECS_SEABORN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SEABORN)

-
  [
     SPECS_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PYTEST)

-
  [
     SPECS_MATPLOTLIB

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_MATPLOTLIB)

-
  [
     SPECS_SPHINX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SPHINX)

-
  [
     SPECS_ASTROPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_ASTROPY)

-
  [
     SPECS_SYMPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SYMPY)

-
  [
     SPECS_PYLINT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PYLINT)

-
  [
     SPECS_XARRAY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_XARRAY)

-
  [
     SPECS_SQLFLUFF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SQLFLUFF)

-
  [
     SPECS_DBT_CORE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_DBT_CORE)

-
  [
     SPECS_PYVISTA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PYVISTA)

-
  [
     SPECS_ASTROID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_ASTROID)

-
  [
     SPECS_MARSHMALLOW

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_MARSHMALLOW)

-
  [
     SPECS_PVLIB

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PVLIB)

-
  [
     SPECS_PYDICOM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PYDICOM)

-
  [
     SPECS_HUMANEVAL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_HUMANEVAL)

-
  [
     MAP_REPO_VERSION_TO_SPECS_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_PY)

-
  [
     MAP_REPO_TO_INSTALL_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_PY)

-
  [
     MAP_REPO_TO_REQS_PATHS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_REQS_PATHS)

-
  [
     MAP_REPO_TO_ENV_YML_PATHS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_ENV_YML_PATHS)

-
  [
     USE_X86_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.USE_X86_PY)

-
  [
     FASTLANE_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FASTLANE_RSPEC_JQ_TRANSFORM)

-
  [
     FPM_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FPM_RSPEC_JQ_TRANSFORM)

-
  [
     RUBOCOP_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.RUBOCOP_RSPEC_JQ_TRANSFORM)

-
  [
     SPECS_JEKYLL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_JEKYLL)

-
  [
     SPECS_FLUENTD

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FLUENTD)

-
  [
     SPECS_FASTLANE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FASTLANE)

-
  [
     SPECS_FPM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FPM)

-
  [
     SPECS_FAKER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FAKER)

-
  [
     SPECS_RUBOCOP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_RUBOCOP)

-
  [
     MAP_REPO_VERSION_TO_SPECS_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_RUBY)

-
  [
     MAP_REPO_TO_INSTALL_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_RUBY)

-
  [
     SPECS_RIPGREP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_RIPGREP)

-
  [
     SPECS_BAT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_BAT)

-
  [
     SPECS_RUFF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_RUFF)

-
  [
     TOKIO_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TOKIO_SPECS)

-
  [
     COREUTILS_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.COREUTILS_SPECS)

-
  [
     NUSHELL_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.NUSHELL_SPECS)

-
  [
     AXUM_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.AXUM_SPECS)

-
  [
     MAP_REPO_VERSION_TO_SPECS_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_RUST)

-
  [
     MAP_REPO_TO_INSTALL_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_RUST)

-
  [
     BASE_IMAGE_BUILD_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.BASE_IMAGE_BUILD_DIR)

-
  [
     ENV_IMAGE_BUILD_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ENV_IMAGE_BUILD_DIR)

-
  [
     INSTANCE_IMAGE_BUILD_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.INSTANCE_IMAGE_BUILD_DIR)

-
  [
     RUN_EVALUATION_LOG_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.RUN_EVALUATION_LOG_DIR)

-
  [
     RUN_VALIDATION_LOG_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.RUN_VALIDATION_LOG_DIR)

-
  [
     FAIL_TO_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FAIL_TO_PASS)

-
  [
     FAIL_TO_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FAIL_TO_FAIL)

-
  [
     PASS_TO_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PASS_TO_PASS)

-
  [
     PASS_TO_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PASS_TO_FAIL)

-
  [
     KEY_INSTANCE_ID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.KEY_INSTANCE_ID)

-
  [
     KEY_MODEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.KEY_MODEL)

-
  [
     KEY_PREDICTION

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.KEY_PREDICTION)

-
  [
     DOCKER_PATCH

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.DOCKER_PATCH)

-
  [
     DOCKER_USER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.DOCKER_USER)

-
  [
     DOCKER_WORKDIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.DOCKER_WORKDIR)

-
  [
     LOG_REPORT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.LOG_REPORT)

-
  [
     LOG_INSTANCE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.LOG_INSTANCE)

-
  [
     LOG_TEST_OUTPUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.LOG_TEST_OUTPUT)

-
  [
     UTF8

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.UTF8)

-
  [
     APPLY_PATCH_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.APPLY_PATCH_FAIL)

-
  [
     APPLY_PATCH_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.APPLY_PATCH_PASS)

-
  [
     INSTALL_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.INSTALL_FAIL)

-
  [
     INSTALL_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.INSTALL_PASS)

-
  [
     INSTALL_TIMEOUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.INSTALL_TIMEOUT)

-
  [
     RESET_FAILED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.RESET_FAILED)

-
  [
     TESTS_ERROR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TESTS_ERROR)

-
  [
     TESTS_FAILED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TESTS_FAILED)

-
  [
     TESTS_PASSED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TESTS_PASSED)

-
  [
     TESTS_TIMEOUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TESTS_TIMEOUT)

-
  [
     START_TEST_OUTPUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.START_TEST_OUTPUT)

-
  [
     END_TEST_OUTPUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.END_TEST_OUTPUT)

-
  [
     NON_TEST_EXTS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.NON_TEST_EXTS)

-
  [
     SWE_BENCH_URL_RAW

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWE_BENCH_URL_RAW)

-
  [
     DEFAULT_DOCKER_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.DEFAULT_DOCKER_SPECS)

-
  [
     FAIL_ONLY_REPOS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FAIL_ONLY_REPOS)

-
  [
     MAP_REPO_VERSION_TO_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS)

-
  [
     MAP_REPO_TO_INSTALL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL)

-
  [
     MAP_REPO_TO_EXT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_EXT)

-
  [
     LATEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.LATEST)

-
  [
     USE_X86

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.USE_X86)

-
  [
     REPO_BASE_COMMIT_BRANCH

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.REPO_BASE_COMMIT_BRANCH)

-
  [
     SWEbenchInstance

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance)

    -
  [
     repo

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.repo)

-
  [
     instance_id

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.instance_id)

-
  [
     base_commit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.base_commit)

-
  [
     patch

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.patch)

-
  [
     test_patch

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.test_patch)

-
  [
     problem_statement

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.problem_statement)

-
  [
     hints_text

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.hints_text)

-
  [
     created_at

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.created_at)

-
  [
     version

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.version)

-
  [
     FAIL_TO_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.FAIL_TO_PASS)

-
  [
     PASS_TO_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.PASS_TO_PASS)

-
  [
     environment_setup_commit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.environment_setup_commit)

-
  [
     ResolvedStatus

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ResolvedStatus)

    -
  [
     NO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ResolvedStatus.NO)

-
  [
     PARTIAL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ResolvedStatus.PARTIAL)

-
  [
     FULL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ResolvedStatus.FULL)

-
  [
     TestStatus

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus)

    -
  [
     FAILED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.FAILED)

-
  [
     PASSED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.PASSED)

-
  [
     SKIPPED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.SKIPPED)

-
  [
     ERROR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.ERROR)

-
  [
     XFAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.XFAIL)

-
  [
     EvalType

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.EvalType)

    -
  [
     PASS_AND_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.EvalType.PASS_AND_FAIL)

-
  [
     FAIL_ONLY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.EvalType.FAIL_ONLY)

-
  [
     PatchType

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType)

    -
  [
     PATCH_GOLD

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_GOLD)

-
  [
     PATCH_PRED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_PRED)

-
  [
     PATCH_PRED_TRY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_PRED_TRY)

-
  [
     PATCH_PRED_MINIMAL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_PRED_MINIMAL)

-
  [
     PATCH_PRED_MINIMAL_TRY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_PRED_MINIMAL_TRY)

-
  [
     PATCH_TEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_TEST)

-
  [
     __str__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.__str__)

-
  [
     make_lombok_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.make_lombok_pre_install_script)

-
  [
     make_lucene_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.make_lucene_pre_install_script)

-
  [
     make_rxjava_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.make_rxjava_pre_install_script)

-
  [
     c

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c)

    -
  [
     SPECS_REDIS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_REDIS)

-
  [
     SPECS_JQ

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_JQ)

-
  [
     SPECS_JSON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_JSON)

-
  [
     SPECS_MICROPYTHON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_MICROPYTHON)

-
  [
     SPECS_VALKEY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_VALKEY)

-
  [
     SPECS_FMT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_FMT)

-
  [
     MAP_REPO_VERSION_TO_SPECS_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.MAP_REPO_VERSION_TO_SPECS_C)

-
  [
     MAP_REPO_TO_INSTALL_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.MAP_REPO_TO_INSTALL_C)

-
  [
     go

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go)

    -
  [
     SPECS_CADDY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_CADDY)

-
  [
     SPECS_TERRAFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_TERRAFORM)

-
  [
     SPECS_PROMETHEUS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_PROMETHEUS)

-
  [
     SPECS_HUGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_HUGO)

-
  [
     SPECS_GIN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_GIN)

-
  [
     MAP_REPO_VERSION_TO_SPECS_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.MAP_REPO_VERSION_TO_SPECS_GO)

-
  [
     MAP_REPO_TO_INSTALL_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.MAP_REPO_TO_INSTALL_GO)

-
  [
     java

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java)

    -
  [
     SPECS_GSON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_GSON)

-
  [
     SPECS_DRUID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_DRUID)

-
  [
     SPECS_JAVAPARSER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_JAVAPARSER)

-
  [
     SPECS_LOMBOK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_LOMBOK)

-
  [
     SPECS_LUCENE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_LUCENE)

-
  [
     SPECS_RXJAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_RXJAVA)

-
  [
     MAP_REPO_VERSION_TO_SPECS_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.MAP_REPO_VERSION_TO_SPECS_JAVA)

-
  [
     MAP_REPO_TO_INSTALL_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.MAP_REPO_TO_INSTALL_JAVA)

-
  [
     make_lombok_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.make_lombok_pre_install_script)

-
  [
     make_lucene_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.make_lucene_pre_install_script)

-
  [
     make_rxjava_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.make_rxjava_pre_install_script)

-
  [
     javascript

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript)

    -
  [
     TEST_XVFB_PREFIX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.TEST_XVFB_PREFIX)

-
  [
     XVFB_DEPS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.XVFB_DEPS)

-
  [
     X11_DEPS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.X11_DEPS)

-
  [
     SPECS_CALYPSO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_CALYPSO)

-
  [
     TEST_CHART_JS_TEMPLATE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.TEST_CHART_JS_TEMPLATE)

-
  [
     SPECS_CHART_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_CHART_JS)

-
  [
     SPECS_MARKED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_MARKED)

-
  [
     SPECS_P5_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_P5_JS)

-
  [
     SPECS_REACT_PDF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_REACT_PDF)

-
  [
     JEST_JSON_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.JEST_JSON_JQ_TRANSFORM)

-
  [
     SPECS_BABEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_BABEL)

-
  [
     SPECS_VUEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_VUEJS)

-
  [
     SPECS_DOCUSAURUS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_DOCUSAURUS)

-
  [
     SPECS_IMMUTABLEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_IMMUTABLEJS)

-
  [
     SPECS_THREEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_THREEJS)

-
  [
     SPECS_PREACT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_PREACT)

-
  [
     SPECS_AXIOS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_AXIOS)

-
  [
     MAP_REPO_VERSION_TO_SPECS_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.MAP_REPO_VERSION_TO_SPECS_JS)

-
  [
     MAP_REPO_TO_INSTALL_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.MAP_REPO_TO_INSTALL_JS)

-
  [
     php

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php)

    -
  [
     SPECS_PHPSPREADSHEET

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.SPECS_PHPSPREADSHEET)

-
  [
     SPECS_LARAVEL_FRAMEWORK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.SPECS_LARAVEL_FRAMEWORK)

-
  [
     SPECS_PHP_CS_FIXER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.SPECS_PHP_CS_FIXER)

-
  [
     SPECS_CARBON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.SPECS_CARBON)

-
  [
     MAP_REPO_VERSION_TO_SPECS_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.MAP_REPO_VERSION_TO_SPECS_PHP)

-
  [
     MAP_REPO_TO_INSTALL_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.MAP_REPO_TO_INSTALL_PHP)

-
  [
     python

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python)

    -
  [
     TEST_ASTROPY_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_ASTROPY_PYTEST)

-
  [
     TEST_DJANGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_DJANGO)

-
  [
     TEST_DJANGO_NO_PARALLEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_DJANGO_NO_PARALLEL)

-
  [
     TEST_SEABORN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SEABORN)

-
  [
     TEST_SEABORN_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SEABORN_VERBOSE)

-
  [
     TEST_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_PYTEST)

-
  [
     TEST_PYTEST_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_PYTEST_VERBOSE)

-
  [
     TEST_SPHINX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SPHINX)

-
  [
     TEST_SYMPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SYMPY)

-
  [
     TEST_SYMPY_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SYMPY_VERBOSE)

-
  [
     SPECS_SKLEARN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SKLEARN)

-
  [
     SPECS_FLASK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_FLASK)

-
  [
     SPECS_DJANGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_DJANGO)

-
  [
     SPECS_REQUESTS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_REQUESTS)

-
  [
     SPECS_SEABORN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SEABORN)

-
  [
     SPECS_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PYTEST)

-
  [
     SPECS_MATPLOTLIB

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_MATPLOTLIB)

-
  [
     SPECS_SPHINX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SPHINX)

-
  [
     SPECS_ASTROPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_ASTROPY)

-
  [
     SPECS_SYMPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SYMPY)

-
  [
     SPECS_PYLINT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PYLINT)

-
  [
     SPECS_XARRAY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_XARRAY)

-
  [
     SPECS_SQLFLUFF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SQLFLUFF)

-
  [
     SPECS_DBT_CORE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_DBT_CORE)

-
  [
     SPECS_PYVISTA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PYVISTA)

-
  [
     SPECS_ASTROID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_ASTROID)

-
  [
     SPECS_MARSHMALLOW

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_MARSHMALLOW)

-
  [
     SPECS_PVLIB

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PVLIB)

-
  [
     SPECS_PYDICOM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PYDICOM)

-
  [
     SPECS_HUMANEVAL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_HUMANEVAL)

-
  [
     MAP_REPO_VERSION_TO_SPECS_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.MAP_REPO_VERSION_TO_SPECS_PY)

-
  [
     MAP_REPO_TO_INSTALL_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.MAP_REPO_TO_INSTALL_PY)

-
  [
     MAP_REPO_TO_REQS_PATHS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.MAP_REPO_TO_REQS_PATHS)

-
  [
     MAP_REPO_TO_ENV_YML_PATHS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.MAP_REPO_TO_ENV_YML_PATHS)

-
  [
     USE_X86_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.USE_X86_PY)

-
  [
     ruby

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby)

    -
  [
     FASTLANE_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.FASTLANE_RSPEC_JQ_TRANSFORM)

-
  [
     FPM_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.FPM_RSPEC_JQ_TRANSFORM)

-
  [
     RUBOCOP_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.RUBOCOP_RSPEC_JQ_TRANSFORM)

-
  [
     SPECS_JEKYLL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_JEKYLL)

-
  [
     SPECS_FLUENTD

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_FLUENTD)

-
  [
     SPECS_FASTLANE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_FASTLANE)

-
  [
     SPECS_FPM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_FPM)

-
  [
     SPECS_FAKER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_FAKER)

-
  [
     SPECS_RUBOCOP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_RUBOCOP)

-
  [
     MAP_REPO_VERSION_TO_SPECS_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.MAP_REPO_VERSION_TO_SPECS_RUBY)

-
  [
     MAP_REPO_TO_INSTALL_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.MAP_REPO_TO_INSTALL_RUBY)

-
  [
     rust

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust)

    -
  [
     SPECS_RIPGREP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.SPECS_RIPGREP)

-
  [
     SPECS_BAT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.SPECS_BAT)

-
  [
     SPECS_RUFF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.SPECS_RUFF)

-
  [
     TOKIO_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.TOKIO_SPECS)

-
  [
     COREUTILS_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.COREUTILS_SPECS)

-
  [
     NUSHELL_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.NUSHELL_SPECS)

-
  [
     AXUM_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.AXUM_SPECS)

-
  [
     MAP_REPO_VERSION_TO_SPECS_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.MAP_REPO_VERSION_TO_SPECS_RUST)

-
  [
     MAP_REPO_TO_INSTALL_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.MAP_REPO_TO_INSTALL_RUST)

-
  [
     docker_build

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build)

    -
  [
     BuildImageError

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError)

    -
  [
     super_str

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.super_str)

-
  [
     image_name

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.image_name)

-
  [
     log_path

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.log_path)

-
  [
     logger

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.logger)

-
  [
     __str__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.__str__)

-
  [
     setup_logger

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.setup_logger)

-
  [
     close_logger

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.close_logger)

-
  [
     build_image

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_image)

-
  [
     build_base_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_base_images)

-
  [
     get_env_configs_to_build

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.get_env_configs_to_build)

-
  [
     build_env_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_env_images)

-
  [
     build_instance_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_instance_images)

-
  [
     build_instance_image

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_instance_image)

-
  [
     build_container

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_container)

-
  [
     docker_utils

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils)

    -
  [
     HEREDOC_DELIMITER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.HEREDOC_DELIMITER)

-
  [
     copy_to_container

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.copy_to_container)

-
  [
     write_to_container

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.write_to_container)

-
  [
     remove_image

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.remove_image)

-
  [
     cleanup_container

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.cleanup_container)

-
  [
     exec_run_with_timeout

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.exec_run_with_timeout)

-
  [
     find_dependent_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.find_dependent_images)

-
  [
     list_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.list_images)

-
  [
     clean_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.clean_images)

-
  [
     should_remove

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.should_remove)

-
  [
     dockerfiles

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles)

    -
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.__all__)

-
  [
     get_dockerfile_base

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.get_dockerfile_base)

-
  [
     get_dockerfile_env

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.get_dockerfile_env)

-
  [
     get_dockerfile_instance

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.get_dockerfile_instance)

-
  [
     c

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.c)

-
  [
     go

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.go)

-
  [
     java

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.java)

-
  [
     javascript

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.javascript)

-
  [
     php

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.php)

-
  [
     python

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.python)

-
  [
     ruby

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.ruby)

-
  [
     rust

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.rust)

-
  [
     grading

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading)

    -
  [
     test_passed

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.test_passed)

-
  [
     test_failed

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.test_failed)

-
  [
     get_logs_eval

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.get_logs_eval)

-
  [
     get_eval_tests_report

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.get_eval_tests_report)

-
  [
     compute_fail_to_pass

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.compute_fail_to_pass)

-
  [
     compute_pass_to_pass

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.compute_pass_to_pass)

-
  [
     get_resolution_status

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.get_resolution_status)

-
  [
     get_eval_report

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.get_eval_report)

-
  [
     log_parsers

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers)

    -
  [
     MAP_REPO_TO_PARSER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.MAP_REPO_TO_PARSER)

-
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.__all__)

-
  [
     c

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c)

    -
  [
     MAP_REPO_TO_PARSER_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.MAP_REPO_TO_PARSER_C)

-
  [
     parse_log_redis

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_redis)

-
  [
     parse_log_jq

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_jq)

-
  [
     parse_log_doctest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_doctest)

-
  [
     parse_log_micropython_test

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_micropython_test)

-
  [
     parse_log_googletest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_googletest)

-
  [
     go

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.go)

    -
  [
     MAP_REPO_TO_PARSER_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.go.MAP_REPO_TO_PARSER_GO)

-
  [
     parse_log_gotest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.go.parse_log_gotest)

-
  [
     java

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java)

    -
  [
     MAP_REPO_TO_PARSER_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java.MAP_REPO_TO_PARSER_JAVA)

-
  [
     parse_log_maven

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java.parse_log_maven)

-
  [
     parse_log_ant

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java.parse_log_ant)

-
  [
     parse_log_gradle_custom

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java.parse_log_gradle_custom)

-
  [
     javascript

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript)

    -
  [
     MAP_REPO_TO_PARSER_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.MAP_REPO_TO_PARSER_JS)

-
  [
     parse_log_calypso

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_calypso)

-
  [
     parse_log_chart_js

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_chart_js)

-
  [
     parse_log_marked

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_marked)

-
  [
     parse_log_p5js

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_p5js)

-
  [
     parse_log_react_pdf

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_react_pdf)

-
  [
     parse_log_jest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_jest)

-
  [
     parse_log_jest_json

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_jest_json)

-
  [
     parse_log_vitest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_vitest)

-
  [
     parse_log_karma

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_karma)

-
  [
     parse_log_tap

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_tap)

-
  [
     parse_log_immutable_js

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_immutable_js)

-
  [
     php

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.php)

    -
  [
     MAP_REPO_TO_PARSER_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.php.MAP_REPO_TO_PARSER_PHP)

-
  [
     parse_log_phpunit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.php.parse_log_phpunit)

-
  [
     python

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python)

    -
  [
     parse_log_astroid

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_astroid)

-
  [
     parse_log_flask

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_flask)

-
  [
     parse_log_marshmallow

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_marshmallow)

-
  [
     parse_log_pvlib

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pvlib)

-
  [
     parse_log_pyvista

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pyvista)

-
  [
     parse_log_sqlfluff

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_sqlfluff)

-
  [
     parse_log_xarray

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_xarray)

-
  [
     parse_log_pydicom

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pydicom)

-
  [
     parse_log_requests

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_requests)

-
  [
     parse_log_pylint

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pylint)

-
  [
     parse_log_astropy

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_astropy)

-
  [
     parse_log_scikit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_scikit)

-
  [
     parse_log_sphinx

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_sphinx)

-
  [
     MAP_REPO_TO_PARSER_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.MAP_REPO_TO_PARSER_PY)

-
  [
     parse_log_pytest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pytest)

-
  [
     parse_log_pytest_options

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pytest_options)

-
  [
     parse_log_django

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_django)

-
  [
     parse_log_pytest_v2

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pytest_v2)

-
  [
     parse_log_seaborn

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_seaborn)

-
  [
     parse_log_sympy

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_sympy)

-
  [
     parse_log_matplotlib

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_matplotlib)

-
  [
     ruby

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby)

    -
  [
     MAP_REPO_TO_PARSER_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.MAP_REPO_TO_PARSER_RUBY)

-
  [
     parse_log_minitest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_minitest)

-
  [
     parse_log_cucumber

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_cucumber)

-
  [
     parse_log_ruby_unit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_ruby_unit)

-
  [
     parse_log_rspec_transformed_json

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_rspec_transformed_json)

-
  [
     parse_log_jekyll

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_jekyll)

-
  [
     rust

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.rust)

    -
  [
     MAP_REPO_TO_PARSER_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.rust.MAP_REPO_TO_PARSER_RUST)

-
  [
     parse_log_cargo

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.rust.parse_log_cargo)

-
  [
     modal_eval

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval)

    -
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.__all__)

-
  [
     run_instances_modal

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_instances_modal)

-
  [
     validate_modal_credentials

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.validate_modal_credentials)

-
  [
     run_evaluation_modal

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal)

    -
  [
     SANDBOX_ENTRYPOINT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.SANDBOX_ENTRYPOINT)

-
  [
     LOCAL_SANDBOX_ENTRYPOINT_PATH

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.LOCAL_SANDBOX_ENTRYPOINT_PATH)

-
  [
     REMOTE_SANDBOX_ENTRYPOINT_PATH

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.REMOTE_SANDBOX_ENTRYPOINT_PATH)

-
  [
     app

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.app)

-
  [
     swebench_image

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.swebench_image)

-
  [
     TestOutput

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.TestOutput)

-
  [
     ModalSandboxRuntime

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.ModalSandboxRuntime)

-
  [
     get_log_dir

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.get_log_dir)

-
  [
     run_instance_modal

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.run_instance_modal)

-
  [
     run_instances_modal

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.run_instances_modal)

-
  [
     run_evaluation_modal_entrypoint

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint)

    -
  [
     STDIO_RATE_LIMIT_BYTES_PER_SEC

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.STDIO_RATE_LIMIT_BYTES_PER_SEC)

-
  [
     parser

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.parser)

-
  [
     args

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.args)

-
  [
     exec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.exec)

-
  [
     main

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.main)

-
  [
     utils

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.utils)

    -
  [
     validate_modal_credentials

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.utils.validate_modal_credentials)

-
  [
     prepare_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images)

    -
  [
     parser

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images.parser)

-
  [
     args

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images.args)

-
  [
     filter_dataset_to_build

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images.filter_dataset_to_build)

-
  [
     main

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images.main)

-
  [
     remove_containers

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers)

    -
  [
     parser

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers.parser)

-
  [
     args

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers.args)

-
  [
     instance_ids

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers.instance_ids)

-
  [
     main

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers.main)

-
  [
     reporting

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.reporting)

    -
  [
     make_run_report

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.reporting.make_run_report)

-
  [
     run_evaluation

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation)

    -
  [
     GIT_APPLY_CMDS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.GIT_APPLY_CMDS)

-
  [
     parser

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.parser)

-
  [
     args

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.args)

-
  [
     run_instance

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.run_instance)

-
  [
     run_instances

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.run_instances)

-
  [
     get_dataset_from_preds

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.get_dataset_from_preds)

-
  [
     main

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.main)

-
  [
     test_spec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec)

    -
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.__all__)

-
  [
     create_scripts

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.create_scripts)

    -
  [
     make_repo_script_list

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.create_scripts.make_repo_script_list)

-
  [
     make_env_script_list

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.create_scripts.make_env_script_list)

-
  [
     make_eval_script_list

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.create_scripts.make_eval_script_list)

-
  [
     javascript

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript)

    -
  [
     MAP_REPO_TO_TEST_CMDS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript.MAP_REPO_TO_TEST_CMDS)

-
  [
     get_test_cmds_calypso

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript.get_test_cmds_calypso)

-
  [
     get_download_img_commands

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript.get_download_img_commands)

-
  [
     make_eval_script_list_js

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript.make_eval_script_list_js)

-
  [
     python

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python)

    -
  [
     HEADERS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.HEADERS)

-
  [
     REPLACE_REQ_PACKAGES

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.REPLACE_REQ_PACKAGES)

-
  [
     get_environment_yml_by_commit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_environment_yml_by_commit)

-
  [
     clean_environment_yml

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.clean_environment_yml)

-
  [
     get_environment_yml

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_environment_yml)

-
  [
     get_requirements_by_commit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_requirements_by_commit)

-
  [
     clean_requirements

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.clean_requirements)

-
  [
     get_requirements

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_requirements)

-
  [
     get_test_directives

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_test_directives)

-
  [
     make_repo_script_list_py

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.make_repo_script_list_py)

-
  [
     make_env_script_list_py_from_conda

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.make_env_script_list_py_from_conda)

-
  [
     make_env_script_list_py

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.make_env_script_list_py)

-
  [
     make_eval_script_list_py

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.make_eval_script_list_py)

-
  [
     test_spec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.test_spec)

    -
  [
     TestSpec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.test_spec.TestSpec)

-
  [
     get_test_specs_from_dataset

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.test_spec.get_test_specs_from_dataset)

-
  [
     make_test_spec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.test_spec.make_test_spec)

-
  [
     utils

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils)

    -
  [
     get_test_cmds

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils.get_test_cmds)

-
  [
     make_repo_script_list_common

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils.make_repo_script_list_common)

-
  [
     make_env_script_list_common

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils.make_env_script_list_common)

-
  [
     make_eval_script_list_common

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils.make_eval_script_list_common)

-
  [
     utils

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils)

    -
  [
     PATCH_PATTERN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.PATCH_PATTERN)

-
  [
     PATCH_FILE_PATTERN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.PATCH_FILE_PATTERN)

-
  [
     PATCH_HUNK_PATTERN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.PATCH_HUNK_PATTERN)

-
  [
     EvaluationError

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError)

    -
  [
     instance_id

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError.instance_id)

-
  [
     log_file

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError.log_file)

-
  [
     logger

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError.logger)

-
  [
     __str__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError.__str__)

-
  [
     get_predictions_from_file

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_predictions_from_file)

-
  [
     run_threadpool

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.run_threadpool)

-
  [
     run_sequential

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.run_sequential)

-
  [
     load_swebench_dataset

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.load_swebench_dataset)

-
  [
     get_first_idx

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_first_idx)

-
  [
     get_last_idx

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_last_idx)

-
  [
     strip_content

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.strip_content)

-
  [
     get_hunk_stats

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_hunk_stats)

-
  [
     extract_minimal_patch

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.extract_minimal_patch)

-
  [
     has_attribute_or_import_error

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.has_attribute_or_import_error)

-
  [
     str2bool

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.str2bool)

-
  [
     optional_str

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.optional_str)

-
  [
     get_repo_file

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_repo_file)

-
  [
     get_modified_files

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_modified_files)

-
  [
     get_new_files

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_new_files)

-
  [
     ansi_escape

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.ansi_escape)

-
  [
     load_cached_environment_yml

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.load_cached_environment_yml)

-
      [

    Inference

      ](https://www.swebench.com/SWE-bench/api/harness/../inference/)

-
      [

    Versioning

      ](https://www.swebench.com/SWE-bench/api/harness/../versioning/)

      Table of contents
    -
  [
     harness

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness)

    -
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.__all__)

-
  [
     constants

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants)

    -
  [
     SPECS_REDIS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_REDIS)

-
  [
     SPECS_JQ

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_JQ)

-
  [
     SPECS_JSON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_JSON)

-
  [
     SPECS_MICROPYTHON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_MICROPYTHON)

-
  [
     SPECS_VALKEY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_VALKEY)

-
  [
     SPECS_FMT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FMT)

-
  [
     MAP_REPO_VERSION_TO_SPECS_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_C)

-
  [
     MAP_REPO_TO_INSTALL_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_C)

-
  [
     SPECS_CADDY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_CADDY)

-
  [
     SPECS_TERRAFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_TERRAFORM)

-
  [
     SPECS_PROMETHEUS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PROMETHEUS)

-
  [
     SPECS_HUGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_HUGO)

-
  [
     SPECS_GIN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_GIN)

-
  [
     MAP_REPO_VERSION_TO_SPECS_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_GO)

-
  [
     MAP_REPO_TO_INSTALL_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_GO)

-
  [
     SPECS_GSON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_GSON)

-
  [
     SPECS_DRUID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_DRUID)

-
  [
     SPECS_JAVAPARSER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_JAVAPARSER)

-
  [
     SPECS_LOMBOK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_LOMBOK)

-
  [
     SPECS_LUCENE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_LUCENE)

-
  [
     SPECS_RXJAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_RXJAVA)

-
  [
     MAP_REPO_VERSION_TO_SPECS_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_JAVA)

-
  [
     MAP_REPO_TO_INSTALL_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_JAVA)

-
  [
     TEST_XVFB_PREFIX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_XVFB_PREFIX)

-
  [
     XVFB_DEPS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.XVFB_DEPS)

-
  [
     X11_DEPS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.X11_DEPS)

-
  [
     SPECS_CALYPSO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_CALYPSO)

-
  [
     TEST_CHART_JS_TEMPLATE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_CHART_JS_TEMPLATE)

-
  [
     SPECS_CHART_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_CHART_JS)

-
  [
     SPECS_MARKED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_MARKED)

-
  [
     SPECS_P5_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_P5_JS)

-
  [
     SPECS_REACT_PDF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_REACT_PDF)

-
  [
     JEST_JSON_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.JEST_JSON_JQ_TRANSFORM)

-
  [
     SPECS_BABEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_BABEL)

-
  [
     SPECS_VUEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_VUEJS)

-
  [
     SPECS_DOCUSAURUS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_DOCUSAURUS)

-
  [
     SPECS_IMMUTABLEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_IMMUTABLEJS)

-
  [
     SPECS_THREEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_THREEJS)

-
  [
     SPECS_PREACT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PREACT)

-
  [
     SPECS_AXIOS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_AXIOS)

-
  [
     MAP_REPO_VERSION_TO_SPECS_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_JS)

-
  [
     MAP_REPO_TO_INSTALL_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_JS)

-
  [
     SPECS_PHPSPREADSHEET

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PHPSPREADSHEET)

-
  [
     SPECS_LARAVEL_FRAMEWORK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_LARAVEL_FRAMEWORK)

-
  [
     SPECS_PHP_CS_FIXER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PHP_CS_FIXER)

-
  [
     SPECS_CARBON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_CARBON)

-
  [
     MAP_REPO_VERSION_TO_SPECS_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_PHP)

-
  [
     MAP_REPO_TO_INSTALL_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_PHP)

-
  [
     TEST_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_PYTEST)

-
  [
     TEST_PYTEST_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_PYTEST_VERBOSE)

-
  [
     TEST_ASTROPY_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_ASTROPY_PYTEST)

-
  [
     TEST_DJANGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_DJANGO)

-
  [
     TEST_DJANGO_NO_PARALLEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_DJANGO_NO_PARALLEL)

-
  [
     TEST_SEABORN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SEABORN)

-
  [
     TEST_SEABORN_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SEABORN_VERBOSE)

-
  [
     TEST_SPHINX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SPHINX)

-
  [
     TEST_SYMPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SYMPY)

-
  [
     TEST_SYMPY_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TEST_SYMPY_VERBOSE)

-
  [
     SPECS_SKLEARN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SKLEARN)

-
  [
     SPECS_FLASK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FLASK)

-
  [
     SPECS_DJANGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_DJANGO)

-
  [
     SPECS_REQUESTS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_REQUESTS)

-
  [
     SPECS_SEABORN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SEABORN)

-
  [
     SPECS_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PYTEST)

-
  [
     SPECS_MATPLOTLIB

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_MATPLOTLIB)

-
  [
     SPECS_SPHINX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SPHINX)

-
  [
     SPECS_ASTROPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_ASTROPY)

-
  [
     SPECS_SYMPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SYMPY)

-
  [
     SPECS_PYLINT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PYLINT)

-
  [
     SPECS_XARRAY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_XARRAY)

-
  [
     SPECS_SQLFLUFF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_SQLFLUFF)

-
  [
     SPECS_DBT_CORE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_DBT_CORE)

-
  [
     SPECS_PYVISTA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PYVISTA)

-
  [
     SPECS_ASTROID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_ASTROID)

-
  [
     SPECS_MARSHMALLOW

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_MARSHMALLOW)

-
  [
     SPECS_PVLIB

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PVLIB)

-
  [
     SPECS_PYDICOM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_PYDICOM)

-
  [
     SPECS_HUMANEVAL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_HUMANEVAL)

-
  [
     MAP_REPO_VERSION_TO_SPECS_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_PY)

-
  [
     MAP_REPO_TO_INSTALL_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_PY)

-
  [
     MAP_REPO_TO_REQS_PATHS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_REQS_PATHS)

-
  [
     MAP_REPO_TO_ENV_YML_PATHS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_ENV_YML_PATHS)

-
  [
     USE_X86_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.USE_X86_PY)

-
  [
     FASTLANE_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FASTLANE_RSPEC_JQ_TRANSFORM)

-
  [
     FPM_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FPM_RSPEC_JQ_TRANSFORM)

-
  [
     RUBOCOP_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.RUBOCOP_RSPEC_JQ_TRANSFORM)

-
  [
     SPECS_JEKYLL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_JEKYLL)

-
  [
     SPECS_FLUENTD

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FLUENTD)

-
  [
     SPECS_FASTLANE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FASTLANE)

-
  [
     SPECS_FPM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FPM)

-
  [
     SPECS_FAKER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_FAKER)

-
  [
     SPECS_RUBOCOP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_RUBOCOP)

-
  [
     MAP_REPO_VERSION_TO_SPECS_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_RUBY)

-
  [
     MAP_REPO_TO_INSTALL_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_RUBY)

-
  [
     SPECS_RIPGREP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_RIPGREP)

-
  [
     SPECS_BAT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_BAT)

-
  [
     SPECS_RUFF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SPECS_RUFF)

-
  [
     TOKIO_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TOKIO_SPECS)

-
  [
     COREUTILS_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.COREUTILS_SPECS)

-
  [
     NUSHELL_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.NUSHELL_SPECS)

-
  [
     AXUM_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.AXUM_SPECS)

-
  [
     MAP_REPO_VERSION_TO_SPECS_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS_RUST)

-
  [
     MAP_REPO_TO_INSTALL_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL_RUST)

-
  [
     BASE_IMAGE_BUILD_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.BASE_IMAGE_BUILD_DIR)

-
  [
     ENV_IMAGE_BUILD_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ENV_IMAGE_BUILD_DIR)

-
  [
     INSTANCE_IMAGE_BUILD_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.INSTANCE_IMAGE_BUILD_DIR)

-
  [
     RUN_EVALUATION_LOG_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.RUN_EVALUATION_LOG_DIR)

-
  [
     RUN_VALIDATION_LOG_DIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.RUN_VALIDATION_LOG_DIR)

-
  [
     FAIL_TO_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FAIL_TO_PASS)

-
  [
     FAIL_TO_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FAIL_TO_FAIL)

-
  [
     PASS_TO_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PASS_TO_PASS)

-
  [
     PASS_TO_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PASS_TO_FAIL)

-
  [
     KEY_INSTANCE_ID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.KEY_INSTANCE_ID)

-
  [
     KEY_MODEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.KEY_MODEL)

-
  [
     KEY_PREDICTION

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.KEY_PREDICTION)

-
  [
     DOCKER_PATCH

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.DOCKER_PATCH)

-
  [
     DOCKER_USER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.DOCKER_USER)

-
  [
     DOCKER_WORKDIR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.DOCKER_WORKDIR)

-
  [
     LOG_REPORT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.LOG_REPORT)

-
  [
     LOG_INSTANCE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.LOG_INSTANCE)

-
  [
     LOG_TEST_OUTPUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.LOG_TEST_OUTPUT)

-
  [
     UTF8

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.UTF8)

-
  [
     APPLY_PATCH_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.APPLY_PATCH_FAIL)

-
  [
     APPLY_PATCH_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.APPLY_PATCH_PASS)

-
  [
     INSTALL_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.INSTALL_FAIL)

-
  [
     INSTALL_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.INSTALL_PASS)

-
  [
     INSTALL_TIMEOUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.INSTALL_TIMEOUT)

-
  [
     RESET_FAILED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.RESET_FAILED)

-
  [
     TESTS_ERROR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TESTS_ERROR)

-
  [
     TESTS_FAILED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TESTS_FAILED)

-
  [
     TESTS_PASSED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TESTS_PASSED)

-
  [
     TESTS_TIMEOUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TESTS_TIMEOUT)

-
  [
     START_TEST_OUTPUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.START_TEST_OUTPUT)

-
  [
     END_TEST_OUTPUT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.END_TEST_OUTPUT)

-
  [
     NON_TEST_EXTS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.NON_TEST_EXTS)

-
  [
     SWE_BENCH_URL_RAW

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWE_BENCH_URL_RAW)

-
  [
     DEFAULT_DOCKER_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.DEFAULT_DOCKER_SPECS)

-
  [
     FAIL_ONLY_REPOS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.FAIL_ONLY_REPOS)

-
  [
     MAP_REPO_VERSION_TO_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_VERSION_TO_SPECS)

-
  [
     MAP_REPO_TO_INSTALL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_INSTALL)

-
  [
     MAP_REPO_TO_EXT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.MAP_REPO_TO_EXT)

-
  [
     LATEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.LATEST)

-
  [
     USE_X86

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.USE_X86)

-
  [
     REPO_BASE_COMMIT_BRANCH

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.REPO_BASE_COMMIT_BRANCH)

-
  [
     SWEbenchInstance

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance)

    -
  [
     repo

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.repo)

-
  [
     instance_id

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.instance_id)

-
  [
     base_commit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.base_commit)

-
  [
     patch

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.patch)

-
  [
     test_patch

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.test_patch)

-
  [
     problem_statement

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.problem_statement)

-
  [
     hints_text

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.hints_text)

-
  [
     created_at

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.created_at)

-
  [
     version

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.version)

-
  [
     FAIL_TO_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.FAIL_TO_PASS)

-
  [
     PASS_TO_PASS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.PASS_TO_PASS)

-
  [
     environment_setup_commit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.SWEbenchInstance.environment_setup_commit)

-
  [
     ResolvedStatus

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ResolvedStatus)

    -
  [
     NO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ResolvedStatus.NO)

-
  [
     PARTIAL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ResolvedStatus.PARTIAL)

-
  [
     FULL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ResolvedStatus.FULL)

-
  [
     TestStatus

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus)

    -
  [
     FAILED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.FAILED)

-
  [
     PASSED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.PASSED)

-
  [
     SKIPPED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.SKIPPED)

-
  [
     ERROR

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.ERROR)

-
  [
     XFAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.TestStatus.XFAIL)

-
  [
     EvalType

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.EvalType)

    -
  [
     PASS_AND_FAIL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.EvalType.PASS_AND_FAIL)

-
  [
     FAIL_ONLY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.EvalType.FAIL_ONLY)

-
  [
     PatchType

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType)

    -
  [
     PATCH_GOLD

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_GOLD)

-
  [
     PATCH_PRED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_PRED)

-
  [
     PATCH_PRED_TRY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_PRED_TRY)

-
  [
     PATCH_PRED_MINIMAL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_PRED_MINIMAL)

-
  [
     PATCH_PRED_MINIMAL_TRY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_PRED_MINIMAL_TRY)

-
  [
     PATCH_TEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.PATCH_TEST)

-
  [
     __str__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.PatchType.__str__)

-
  [
     make_lombok_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.make_lombok_pre_install_script)

-
  [
     make_lucene_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.make_lucene_pre_install_script)

-
  [
     make_rxjava_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.make_rxjava_pre_install_script)

-
  [
     c

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c)

    -
  [
     SPECS_REDIS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_REDIS)

-
  [
     SPECS_JQ

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_JQ)

-
  [
     SPECS_JSON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_JSON)

-
  [
     SPECS_MICROPYTHON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_MICROPYTHON)

-
  [
     SPECS_VALKEY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_VALKEY)

-
  [
     SPECS_FMT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.SPECS_FMT)

-
  [
     MAP_REPO_VERSION_TO_SPECS_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.MAP_REPO_VERSION_TO_SPECS_C)

-
  [
     MAP_REPO_TO_INSTALL_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.c.MAP_REPO_TO_INSTALL_C)

-
  [
     go

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go)

    -
  [
     SPECS_CADDY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_CADDY)

-
  [
     SPECS_TERRAFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_TERRAFORM)

-
  [
     SPECS_PROMETHEUS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_PROMETHEUS)

-
  [
     SPECS_HUGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_HUGO)

-
  [
     SPECS_GIN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.SPECS_GIN)

-
  [
     MAP_REPO_VERSION_TO_SPECS_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.MAP_REPO_VERSION_TO_SPECS_GO)

-
  [
     MAP_REPO_TO_INSTALL_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.go.MAP_REPO_TO_INSTALL_GO)

-
  [
     java

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java)

    -
  [
     SPECS_GSON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_GSON)

-
  [
     SPECS_DRUID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_DRUID)

-
  [
     SPECS_JAVAPARSER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_JAVAPARSER)

-
  [
     SPECS_LOMBOK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_LOMBOK)

-
  [
     SPECS_LUCENE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_LUCENE)

-
  [
     SPECS_RXJAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.SPECS_RXJAVA)

-
  [
     MAP_REPO_VERSION_TO_SPECS_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.MAP_REPO_VERSION_TO_SPECS_JAVA)

-
  [
     MAP_REPO_TO_INSTALL_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.MAP_REPO_TO_INSTALL_JAVA)

-
  [
     make_lombok_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.make_lombok_pre_install_script)

-
  [
     make_lucene_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.make_lucene_pre_install_script)

-
  [
     make_rxjava_pre_install_script

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.java.make_rxjava_pre_install_script)

-
  [
     javascript

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript)

    -
  [
     TEST_XVFB_PREFIX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.TEST_XVFB_PREFIX)

-
  [
     XVFB_DEPS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.XVFB_DEPS)

-
  [
     X11_DEPS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.X11_DEPS)

-
  [
     SPECS_CALYPSO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_CALYPSO)

-
  [
     TEST_CHART_JS_TEMPLATE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.TEST_CHART_JS_TEMPLATE)

-
  [
     SPECS_CHART_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_CHART_JS)

-
  [
     SPECS_MARKED

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_MARKED)

-
  [
     SPECS_P5_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_P5_JS)

-
  [
     SPECS_REACT_PDF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_REACT_PDF)

-
  [
     JEST_JSON_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.JEST_JSON_JQ_TRANSFORM)

-
  [
     SPECS_BABEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_BABEL)

-
  [
     SPECS_VUEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_VUEJS)

-
  [
     SPECS_DOCUSAURUS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_DOCUSAURUS)

-
  [
     SPECS_IMMUTABLEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_IMMUTABLEJS)

-
  [
     SPECS_THREEJS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_THREEJS)

-
  [
     SPECS_PREACT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_PREACT)

-
  [
     SPECS_AXIOS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.SPECS_AXIOS)

-
  [
     MAP_REPO_VERSION_TO_SPECS_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.MAP_REPO_VERSION_TO_SPECS_JS)

-
  [
     MAP_REPO_TO_INSTALL_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.javascript.MAP_REPO_TO_INSTALL_JS)

-
  [
     php

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php)

    -
  [
     SPECS_PHPSPREADSHEET

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.SPECS_PHPSPREADSHEET)

-
  [
     SPECS_LARAVEL_FRAMEWORK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.SPECS_LARAVEL_FRAMEWORK)

-
  [
     SPECS_PHP_CS_FIXER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.SPECS_PHP_CS_FIXER)

-
  [
     SPECS_CARBON

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.SPECS_CARBON)

-
  [
     MAP_REPO_VERSION_TO_SPECS_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.MAP_REPO_VERSION_TO_SPECS_PHP)

-
  [
     MAP_REPO_TO_INSTALL_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.php.MAP_REPO_TO_INSTALL_PHP)

-
  [
     python

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python)

    -
  [
     TEST_ASTROPY_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_ASTROPY_PYTEST)

-
  [
     TEST_DJANGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_DJANGO)

-
  [
     TEST_DJANGO_NO_PARALLEL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_DJANGO_NO_PARALLEL)

-
  [
     TEST_SEABORN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SEABORN)

-
  [
     TEST_SEABORN_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SEABORN_VERBOSE)

-
  [
     TEST_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_PYTEST)

-
  [
     TEST_PYTEST_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_PYTEST_VERBOSE)

-
  [
     TEST_SPHINX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SPHINX)

-
  [
     TEST_SYMPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SYMPY)

-
  [
     TEST_SYMPY_VERBOSE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.TEST_SYMPY_VERBOSE)

-
  [
     SPECS_SKLEARN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SKLEARN)

-
  [
     SPECS_FLASK

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_FLASK)

-
  [
     SPECS_DJANGO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_DJANGO)

-
  [
     SPECS_REQUESTS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_REQUESTS)

-
  [
     SPECS_SEABORN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SEABORN)

-
  [
     SPECS_PYTEST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PYTEST)

-
  [
     SPECS_MATPLOTLIB

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_MATPLOTLIB)

-
  [
     SPECS_SPHINX

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SPHINX)

-
  [
     SPECS_ASTROPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_ASTROPY)

-
  [
     SPECS_SYMPY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SYMPY)

-
  [
     SPECS_PYLINT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PYLINT)

-
  [
     SPECS_XARRAY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_XARRAY)

-
  [
     SPECS_SQLFLUFF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_SQLFLUFF)

-
  [
     SPECS_DBT_CORE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_DBT_CORE)

-
  [
     SPECS_PYVISTA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PYVISTA)

-
  [
     SPECS_ASTROID

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_ASTROID)

-
  [
     SPECS_MARSHMALLOW

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_MARSHMALLOW)

-
  [
     SPECS_PVLIB

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PVLIB)

-
  [
     SPECS_PYDICOM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_PYDICOM)

-
  [
     SPECS_HUMANEVAL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.SPECS_HUMANEVAL)

-
  [
     MAP_REPO_VERSION_TO_SPECS_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.MAP_REPO_VERSION_TO_SPECS_PY)

-
  [
     MAP_REPO_TO_INSTALL_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.MAP_REPO_TO_INSTALL_PY)

-
  [
     MAP_REPO_TO_REQS_PATHS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.MAP_REPO_TO_REQS_PATHS)

-
  [
     MAP_REPO_TO_ENV_YML_PATHS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.MAP_REPO_TO_ENV_YML_PATHS)

-
  [
     USE_X86_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.python.USE_X86_PY)

-
  [
     ruby

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby)

    -
  [
     FASTLANE_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.FASTLANE_RSPEC_JQ_TRANSFORM)

-
  [
     FPM_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.FPM_RSPEC_JQ_TRANSFORM)

-
  [
     RUBOCOP_RSPEC_JQ_TRANSFORM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.RUBOCOP_RSPEC_JQ_TRANSFORM)

-
  [
     SPECS_JEKYLL

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_JEKYLL)

-
  [
     SPECS_FLUENTD

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_FLUENTD)

-
  [
     SPECS_FASTLANE

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_FASTLANE)

-
  [
     SPECS_FPM

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_FPM)

-
  [
     SPECS_FAKER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_FAKER)

-
  [
     SPECS_RUBOCOP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.SPECS_RUBOCOP)

-
  [
     MAP_REPO_VERSION_TO_SPECS_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.MAP_REPO_VERSION_TO_SPECS_RUBY)

-
  [
     MAP_REPO_TO_INSTALL_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.ruby.MAP_REPO_TO_INSTALL_RUBY)

-
  [
     rust

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust)

    -
  [
     SPECS_RIPGREP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.SPECS_RIPGREP)

-
  [
     SPECS_BAT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.SPECS_BAT)

-
  [
     SPECS_RUFF

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.SPECS_RUFF)

-
  [
     TOKIO_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.TOKIO_SPECS)

-
  [
     COREUTILS_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.COREUTILS_SPECS)

-
  [
     NUSHELL_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.NUSHELL_SPECS)

-
  [
     AXUM_SPECS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.AXUM_SPECS)

-
  [
     MAP_REPO_VERSION_TO_SPECS_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.MAP_REPO_VERSION_TO_SPECS_RUST)

-
  [
     MAP_REPO_TO_INSTALL_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.constants.rust.MAP_REPO_TO_INSTALL_RUST)

-
  [
     docker_build

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build)

    -
  [
     BuildImageError

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError)

    -
  [
     super_str

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.super_str)

-
  [
     image_name

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.image_name)

-
  [
     log_path

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.log_path)

-
  [
     logger

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.logger)

-
  [
     __str__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.BuildImageError.__str__)

-
  [
     setup_logger

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.setup_logger)

-
  [
     close_logger

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.close_logger)

-
  [
     build_image

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_image)

-
  [
     build_base_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_base_images)

-
  [
     get_env_configs_to_build

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.get_env_configs_to_build)

-
  [
     build_env_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_env_images)

-
  [
     build_instance_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_instance_images)

-
  [
     build_instance_image

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_instance_image)

-
  [
     build_container

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_build.build_container)

-
  [
     docker_utils

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils)

    -
  [
     HEREDOC_DELIMITER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.HEREDOC_DELIMITER)

-
  [
     copy_to_container

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.copy_to_container)

-
  [
     write_to_container

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.write_to_container)

-
  [
     remove_image

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.remove_image)

-
  [
     cleanup_container

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.cleanup_container)

-
  [
     exec_run_with_timeout

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.exec_run_with_timeout)

-
  [
     find_dependent_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.find_dependent_images)

-
  [
     list_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.list_images)

-
  [
     clean_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.clean_images)

-
  [
     should_remove

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.docker_utils.should_remove)

-
  [
     dockerfiles

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles)

    -
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.__all__)

-
  [
     get_dockerfile_base

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.get_dockerfile_base)

-
  [
     get_dockerfile_env

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.get_dockerfile_env)

-
  [
     get_dockerfile_instance

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.get_dockerfile_instance)

-
  [
     c

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.c)

-
  [
     go

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.go)

-
  [
     java

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.java)

-
  [
     javascript

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.javascript)

-
  [
     php

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.php)

-
  [
     python

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.python)

-
  [
     ruby

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.ruby)

-
  [
     rust

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.dockerfiles.rust)

-
  [
     grading

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading)

    -
  [
     test_passed

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.test_passed)

-
  [
     test_failed

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.test_failed)

-
  [
     get_logs_eval

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.get_logs_eval)

-
  [
     get_eval_tests_report

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.get_eval_tests_report)

-
  [
     compute_fail_to_pass

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.compute_fail_to_pass)

-
  [
     compute_pass_to_pass

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.compute_pass_to_pass)

-
  [
     get_resolution_status

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.get_resolution_status)

-
  [
     get_eval_report

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.grading.get_eval_report)

-
  [
     log_parsers

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers)

    -
  [
     MAP_REPO_TO_PARSER

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.MAP_REPO_TO_PARSER)

-
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.__all__)

-
  [
     c

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c)

    -
  [
     MAP_REPO_TO_PARSER_C

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.MAP_REPO_TO_PARSER_C)

-
  [
     parse_log_redis

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_redis)

-
  [
     parse_log_jq

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_jq)

-
  [
     parse_log_doctest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_doctest)

-
  [
     parse_log_micropython_test

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_micropython_test)

-
  [
     parse_log_googletest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.c.parse_log_googletest)

-
  [
     go

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.go)

    -
  [
     MAP_REPO_TO_PARSER_GO

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.go.MAP_REPO_TO_PARSER_GO)

-
  [
     parse_log_gotest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.go.parse_log_gotest)

-
  [
     java

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java)

    -
  [
     MAP_REPO_TO_PARSER_JAVA

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java.MAP_REPO_TO_PARSER_JAVA)

-
  [
     parse_log_maven

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java.parse_log_maven)

-
  [
     parse_log_ant

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java.parse_log_ant)

-
  [
     parse_log_gradle_custom

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.java.parse_log_gradle_custom)

-
  [
     javascript

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript)

    -
  [
     MAP_REPO_TO_PARSER_JS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.MAP_REPO_TO_PARSER_JS)

-
  [
     parse_log_calypso

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_calypso)

-
  [
     parse_log_chart_js

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_chart_js)

-
  [
     parse_log_marked

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_marked)

-
  [
     parse_log_p5js

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_p5js)

-
  [
     parse_log_react_pdf

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_react_pdf)

-
  [
     parse_log_jest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_jest)

-
  [
     parse_log_jest_json

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_jest_json)

-
  [
     parse_log_vitest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_vitest)

-
  [
     parse_log_karma

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_karma)

-
  [
     parse_log_tap

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_tap)

-
  [
     parse_log_immutable_js

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.javascript.parse_log_immutable_js)

-
  [
     php

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.php)

    -
  [
     MAP_REPO_TO_PARSER_PHP

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.php.MAP_REPO_TO_PARSER_PHP)

-
  [
     parse_log_phpunit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.php.parse_log_phpunit)

-
  [
     python

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python)

    -
  [
     parse_log_astroid

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_astroid)

-
  [
     parse_log_flask

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_flask)

-
  [
     parse_log_marshmallow

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_marshmallow)

-
  [
     parse_log_pvlib

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pvlib)

-
  [
     parse_log_pyvista

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pyvista)

-
  [
     parse_log_sqlfluff

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_sqlfluff)

-
  [
     parse_log_xarray

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_xarray)

-
  [
     parse_log_pydicom

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pydicom)

-
  [
     parse_log_requests

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_requests)

-
  [
     parse_log_pylint

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pylint)

-
  [
     parse_log_astropy

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_astropy)

-
  [
     parse_log_scikit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_scikit)

-
  [
     parse_log_sphinx

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_sphinx)

-
  [
     MAP_REPO_TO_PARSER_PY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.MAP_REPO_TO_PARSER_PY)

-
  [
     parse_log_pytest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pytest)

-
  [
     parse_log_pytest_options

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pytest_options)

-
  [
     parse_log_django

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_django)

-
  [
     parse_log_pytest_v2

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_pytest_v2)

-
  [
     parse_log_seaborn

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_seaborn)

-
  [
     parse_log_sympy

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_sympy)

-
  [
     parse_log_matplotlib

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.python.parse_log_matplotlib)

-
  [
     ruby

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby)

    -
  [
     MAP_REPO_TO_PARSER_RUBY

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.MAP_REPO_TO_PARSER_RUBY)

-
  [
     parse_log_minitest

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_minitest)

-
  [
     parse_log_cucumber

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_cucumber)

-
  [
     parse_log_ruby_unit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_ruby_unit)

-
  [
     parse_log_rspec_transformed_json

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_rspec_transformed_json)

-
  [
     parse_log_jekyll

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.ruby.parse_log_jekyll)

-
  [
     rust

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.rust)

    -
  [
     MAP_REPO_TO_PARSER_RUST

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.rust.MAP_REPO_TO_PARSER_RUST)

-
  [
     parse_log_cargo

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.log_parsers.rust.parse_log_cargo)

-
  [
     modal_eval

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval)

    -
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.__all__)

-
  [
     run_instances_modal

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_instances_modal)

-
  [
     validate_modal_credentials

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.validate_modal_credentials)

-
  [
     run_evaluation_modal

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal)

    -
  [
     SANDBOX_ENTRYPOINT

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.SANDBOX_ENTRYPOINT)

-
  [
     LOCAL_SANDBOX_ENTRYPOINT_PATH

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.LOCAL_SANDBOX_ENTRYPOINT_PATH)

-
  [
     REMOTE_SANDBOX_ENTRYPOINT_PATH

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.REMOTE_SANDBOX_ENTRYPOINT_PATH)

-
  [
     app

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.app)

-
  [
     swebench_image

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.swebench_image)

-
  [
     TestOutput

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.TestOutput)

-
  [
     ModalSandboxRuntime

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.ModalSandboxRuntime)

-
  [
     get_log_dir

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.get_log_dir)

-
  [
     run_instance_modal

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.run_instance_modal)

-
  [
     run_instances_modal

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal.run_instances_modal)

-
  [
     run_evaluation_modal_entrypoint

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint)

    -
  [
     STDIO_RATE_LIMIT_BYTES_PER_SEC

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.STDIO_RATE_LIMIT_BYTES_PER_SEC)

-
  [
     parser

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.parser)

-
  [
     args

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.args)

-
  [
     exec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.exec)

-
  [
     main

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.run_evaluation_modal_entrypoint.main)

-
  [
     utils

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.utils)

    -
  [
     validate_modal_credentials

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.modal_eval.utils.validate_modal_credentials)

-
  [
     prepare_images

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images)

    -
  [
     parser

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images.parser)

-
  [
     args

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images.args)

-
  [
     filter_dataset_to_build

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images.filter_dataset_to_build)

-
  [
     main

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.prepare_images.main)

-
  [
     remove_containers

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers)

    -
  [
     parser

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers.parser)

-
  [
     args

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers.args)

-
  [
     instance_ids

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers.instance_ids)

-
  [
     main

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.remove_containers.main)

-
  [
     reporting

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.reporting)

    -
  [
     make_run_report

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.reporting.make_run_report)

-
  [
     run_evaluation

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation)

    -
  [
     GIT_APPLY_CMDS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.GIT_APPLY_CMDS)

-
  [
     parser

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.parser)

-
  [
     args

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.args)

-
  [
     run_instance

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.run_instance)

-
  [
     run_instances

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.run_instances)

-
  [
     get_dataset_from_preds

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.get_dataset_from_preds)

-
  [
     main

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.run_evaluation.main)

-
  [
     test_spec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec)

    -
  [
     __all__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.__all__)

-
  [
     create_scripts

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.create_scripts)

    -
  [
     make_repo_script_list

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.create_scripts.make_repo_script_list)

-
  [
     make_env_script_list

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.create_scripts.make_env_script_list)

-
  [
     make_eval_script_list

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.create_scripts.make_eval_script_list)

-
  [
     javascript

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript)

    -
  [
     MAP_REPO_TO_TEST_CMDS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript.MAP_REPO_TO_TEST_CMDS)

-
  [
     get_test_cmds_calypso

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript.get_test_cmds_calypso)

-
  [
     get_download_img_commands

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript.get_download_img_commands)

-
  [
     make_eval_script_list_js

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.javascript.make_eval_script_list_js)

-
  [
     python

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python)

    -
  [
     HEADERS

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.HEADERS)

-
  [
     REPLACE_REQ_PACKAGES

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.REPLACE_REQ_PACKAGES)

-
  [
     get_environment_yml_by_commit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_environment_yml_by_commit)

-
  [
     clean_environment_yml

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.clean_environment_yml)

-
  [
     get_environment_yml

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_environment_yml)

-
  [
     get_requirements_by_commit

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_requirements_by_commit)

-
  [
     clean_requirements

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.clean_requirements)

-
  [
     get_requirements

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_requirements)

-
  [
     get_test_directives

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.get_test_directives)

-
  [
     make_repo_script_list_py

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.make_repo_script_list_py)

-
  [
     make_env_script_list_py_from_conda

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.make_env_script_list_py_from_conda)

-
  [
     make_env_script_list_py

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.make_env_script_list_py)

-
  [
     make_eval_script_list_py

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.python.make_eval_script_list_py)

-
  [
     test_spec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.test_spec)

    -
  [
     TestSpec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.test_spec.TestSpec)

-
  [
     get_test_specs_from_dataset

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.test_spec.get_test_specs_from_dataset)

-
  [
     make_test_spec

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.test_spec.make_test_spec)

-
  [
     utils

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils)

    -
  [
     get_test_cmds

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils.get_test_cmds)

-
  [
     make_repo_script_list_common

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils.make_repo_script_list_common)

-
  [
     make_env_script_list_common

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils.make_env_script_list_common)

-
  [
     make_eval_script_list_common

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.test_spec.utils.make_eval_script_list_common)

-
  [
     utils

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils)

    -
  [
     PATCH_PATTERN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.PATCH_PATTERN)

-
  [
     PATCH_FILE_PATTERN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.PATCH_FILE_PATTERN)

-
  [
     PATCH_HUNK_PATTERN

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.PATCH_HUNK_PATTERN)

-
  [
     EvaluationError

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError)

    -
  [
     instance_id

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError.instance_id)

-
  [
     log_file

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError.log_file)

-
  [
     logger

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError.logger)

-
  [
     __str__

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.EvaluationError.__str__)

-
  [
     get_predictions_from_file

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_predictions_from_file)

-
  [
     run_threadpool

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.run_threadpool)

-
  [
     run_sequential

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.run_sequential)

-
  [
     load_swebench_dataset

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.load_swebench_dataset)

-
  [
     get_first_idx

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_first_idx)

-
  [
     get_last_idx

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_last_idx)

-
  [
     strip_content

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.strip_content)

-
  [
     get_hunk_stats

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_hunk_stats)

-
  [
     extract_minimal_patch

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.extract_minimal_patch)

-
  [
     has_attribute_or_import_error

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.has_attribute_or_import_error)

-
  [
     str2bool

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.str2bool)

-
  [
     optional_str

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.optional_str)

-
  [
     get_repo_file

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_repo_file)

-
  [
     get_modified_files

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_modified_files)

-
  [
     get_new_files

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.get_new_files)

-
  [
     ansi_escape

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.ansi_escape)

-
  [
     load_cached_environment_yml

  ](https://www.swebench.com/SWE-bench/api/harness/#swebench.harness.utils.load_cached_environment_yml)

[

    ](https://github.com/SWE-bench/SWE-bench/edit/main/docs/api/harness.md) # Harness API

###
            swebench.harness

####
            __all__

  `module-attribute`

```
__all__ = ['docker_build', 'docker_utils', 'grading', 'prepare_images', 'remove_containers', 'reporting', 'run_evaluation', 'utils', 'constants', 'dockerfiles', 'log_parsers', 'modal_eval', 'test_spec']
```

####
            constants

#####
            SPECS_REDIS

  `module-attribute`

```
SPECS_REDIS = {'13115': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/scripting']}, '12472': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/acl --only "/.*ACL GETUSER.*"']}, '12272': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/string --only "/.*(GETRANGE|SETRANGE).*"']}, '11734': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/bitops']}, '10764': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/zset --only "BZMPOP"']}, '10095': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/list --only "/.*(LPOP|RPOP)"']}, '9733': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/introspection-2']}, '10068': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/stream --only "/*XTRIM*"']}, '11631': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/geo --only "/.*GEOSEARCH .*"']}, '11510': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/introspection --only "/.*MONITOR.*"']}, '11279': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/acl']}, '13338': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/stream-cgroups']}}
```

#####
            SPECS_JQ

  `module-attribute`

```
SPECS_JQ = {None: {k: {'build': ['git submodule update --init', 'autoreconf -fi', './configure --with-oniguruma=builtin', 'make clean', 'touch src/parser.y src/lexer.l', 'make -j$(nproc)'], 'test_cmd': ['make check']} for k in ['2839', '2650', '2235', '2658', '2750', '2681', '2919', '2598', '2728']}}
```

#####
            SPECS_JSON

  `module-attribute`

```
SPECS_JSON = {'4237': {'build': ['mkdir -p build', 'cd build', 'cmake ..', 'make test-udt_cpp11', 'cd ..'], 'test_cmd': ['./build/tests/test-udt_cpp11 -s -r=xml']}}
```

#####
            SPECS_MICROPYTHON

  `module-attribute`

```
SPECS_MICROPYTHON = {'15898': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate'], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard', 'gcc -shared -o tests/ports/unix/ffi_lib.so tests/ports/unix/ffi_lib.c'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -i string_format']}, '13569': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate'], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard', 'gcc -shared -o tests/ports/unix/ffi_lib.so tests/ports/unix/ffi_lib.c'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -i try']}, '13039': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate'], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard', 'gcc -shared -o tests/unix/ffi_lib.so tests/unix/ffi_lib.c'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -i slice']}, '12158': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate'], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard', 'gcc -shared -o tests/unix/ffi_lib.so tests/unix/ffi_lib.c'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -d thread']}, '10095': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate', "sed -i 's/uint mp_import_stat/mp_import_stat_t mp_import_stat/' mpy-cross/main.c"], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -i basics/fun']}}
```

#####
            SPECS_VALKEY

  `module-attribute`

```
SPECS_VALKEY = {'928': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/cluster/replica-migration --only "/.*NOREPLICAS.*"']}, '790': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/cluster/cluster-shards']}, '1499': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/introspection-2']}, '1842': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/acl --only "/.*ACL LOAD.*"']}}
```

#####
            SPECS_FMT

  `module-attribute`

```
SPECS_FMT = {None: {k: {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target ranges-test'], 'test_cmd': ['ctest --test-dir build -V -R ranges-test']} for k in ['3863', '3158', '2457']}, None: {k: {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target format-test'], 'test_cmd': ['ctest --test-dir build -V -R format-test']} for k in ['3901', '3750', '3248', '2317', '2310']}, '3272': {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target xchar-test'], 'test_cmd': ['ctest --test-dir build -V -R xchar-test']}, '3729': {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target std-test'], 'test_cmd': ['ctest --test-dir build -V -R std-test']}, '1683': {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target printf-test'], 'test_cmd': ['ctest --test-dir build -V -R printf-test']}}
```

#####
            MAP_REPO_VERSION_TO_SPECS_C

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_C = {'redis/redis': SPECS_REDIS, 'jqlang/jq': SPECS_JQ, 'nlohmann/json': SPECS_JSON, 'micropython/micropython': SPECS_MICROPYTHON, 'valkey-io/valkey': SPECS_VALKEY, 'fmtlib/fmt': SPECS_FMT}
```

#####
            MAP_REPO_TO_INSTALL_C

  `module-attribute`

```
MAP_REPO_TO_INSTALL_C = {}
```

#####
            SPECS_CADDY

  `module-attribute`

```
SPECS_CADDY = {'6411': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go mod tidy'], 'test_cmd': ['go test -v . -run "TestReplacerNew*"']}, '6345': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddytest/integration'], 'test_cmd': ['go test -v ./caddytest/integration']}, '6115': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./modules/caddyhttp/reverseproxy'], 'test_cmd': ['go test -v ./modules/caddyhttp/reverseproxy']}, '6051': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddyconfig/caddyfile'], 'test_cmd': ['go test -v ./caddyconfig/caddyfile']}, '5404': {'docker_specs': {'go_version': '1.20.14'}, 'install': ['go test -c ./caddyconfig/caddyfile'], 'test_cmd': ['go test -v ./caddyconfig/caddyfile']}, '6370': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./cmd'], 'test_cmd': ['go test -v ./cmd']}, '6350': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddytest/integration -run "TestCaddyfileAdapt*"'], 'test_cmd': ['go test -v ./caddytest/integration -run "TestCaddyfileAdapt*"']}, '6288': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddytest/integration -run "TestCaddyfileAdapt*"'], 'test_cmd': ['go test -v ./caddytest/integration -run "TestCaddyfileAdapt*"']}, '5995': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddytest/integration -run "^TestUriReplace"'], 'test_cmd': ['go test -v ./caddytest/integration -run "^TestUriReplace"']}, '4943': {'docker_specs': {'go_version': '1.18.10'}, 'install': ['go test -c ./modules/logging'], 'test_cmd': ['go test -v ./modules/logging']}, '5626': {'docker_specs': {'go_version': '1.19.13'}, 'install': ['go test -c ./caddyconfig/httpcaddyfile -run "Test.*Import"'], 'test_cmd': ['go test -v ./caddyconfig/httpcaddyfile -run "Test.*Import"']}, '5761': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddyconfig/caddyfile -run "TestLexer.*"'], 'test_cmd': ['go test -v ./caddyconfig/caddyfile -run "TestLexer.*"']}, '5870': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c . -run "TestUnsyncedConfigAccess"'], 'test_cmd': ['go test -v . -run "TestUnsyncedConfigAccess"']}, '4774': {'docker_specs': {'go_version': '1.18.10'}, 'install': ['go test -c ./caddytest/integration -run "TestCaddyfileAdapt*"'], 'test_cmd': ['go test -v ./caddytest/integration -run "TestCaddyfileAdapt*"']}}
```

#####
            SPECS_TERRAFORM

  `module-attribute`

```
SPECS_TERRAFORM = {'35611': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./internal/terraform'], 'test_cmd': ['go test -v ./internal/terraform -run "^TestContext2Apply_provisioner"']}, '35543': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./internal/terraform'], 'test_cmd': ['go test -v ./internal/terraform -run "^TestContext2Plan_import"']}, '34900': {'docker_specs': {'go_version': '1.22.12'}, 'install': ['go test -c ./internal/terraform'], 'test_cmd': ['go test -v ./internal/terraform -run "(^TestContext2Apply|^TestContext2Plan).*[Ss]ensitive"']}, '34580': {'docker_specs': {'go_version': '1.21.13'}, 'install': ['go test -c ./internal/command'], 'test_cmd': ['go test -v ./internal/command -run "^TestFmt"']}, '34814': {'docker_specs': {'go_version': '1.22.12'}, 'install': ['go test -c ./internal/builtin/provisioners/remote-exec'], 'test_cmd': ['go test -v ./internal/builtin/provisioners/remote-exec']}}
```

#####
            SPECS_PROMETHEUS

  `module-attribute`

```
SPECS_PROMETHEUS = {'14861': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./promql'], 'test_cmd': ['go test -v ./promql -run "^TestEngine"']}, '13845': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./promql ./model/labels'], 'test_cmd': ['go test -v ./promql ./model/labels -run "^(TestRangeQuery|TestLabels)"']}, '12874': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./tsdb'], 'test_cmd': ['go test -v ./tsdb -run "^TestHead"']}, '11859': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./tsdb'], 'test_cmd': ['go test -v ./tsdb -run "^TestSnapshot"']}, '10720': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./promql'], 'test_cmd': ['go test -v ./promql -run "^TestEvaluations"']}, '10633': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./discovery/puppetdb'], 'test_cmd': ['go test -v ./discovery/puppetdb -run "TestPuppetDBRefreshWithParameters"']}, '9248': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./promql'], 'test_cmd': ['go test -v ./promql -run "^TestEvaluations"']}, '15142': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./tsdb'], 'test_cmd': ['go test -v ./tsdb -run "^TestHead"']}}
```

#####
            SPECS_HUGO

  `module-attribute`

```
SPECS_HUGO = {'12768': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./markup/goldmark/blockquotes/...'], 'test_cmd': ['go test -v ./markup/goldmark/blockquotes/...']}, '12579': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./resources/page'], 'test_cmd': ['go test -v ./resources/page -run "^TestGroupBy"']}, '12562': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./hugolib/...'], 'test_cmd': ['go test -v ./hugolib/... -run "^TestGetPage[^/]"']}, '12448': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./hugolib/...'], 'test_cmd': ['go test -v ./hugolib/... -run "^TestRebuild"']}, '12343': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./resources/page/...'], 'test_cmd': ['go test -v ./resources/page/... -run "^Test.*Permalink"']}, '12204': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./tpl/tplimpl'], 'test_cmd': ['go test -v ./tpl/tplimpl -run "^TestEmbedded"']}, '12171': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./hugolib'], 'test_cmd': ['go test -v ./hugolib -run "^Test.*Pages"']}}
```

#####
            SPECS_GIN

  `module-attribute`

```
SPECS_GIN = {'4003': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test . -v -run "TestMethodNotAllowedNoRoute"']}, '3820': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./binding'], 'test_cmd': ['go test -v ./binding -run "^TestMapping"']}, '3741': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^TestColor"']}, '2755': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^TestTree"']}, '3227': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^TestRedirect"']}, '2121': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./...'], 'test_cmd': ['go test -v ./... -run "^Test.*Reader"']}, '1957': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^TestContext.*Bind"']}, '1805': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^Test.*Router"']}}
```

#####
            MAP_REPO_VERSION_TO_SPECS_GO

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_GO = {'caddyserver/caddy': SPECS_CADDY, 'hashicorp/terraform': SPECS_TERRAFORM, 'prometheus/prometheus': SPECS_PROMETHEUS, 'gohugoio/hugo': SPECS_HUGO, 'gin-gonic/gin': SPECS_GIN}
```

#####
            MAP_REPO_TO_INSTALL_GO

  `module-attribute`

```
MAP_REPO_TO_INSTALL_GO = {}
```

#####
            SPECS_GSON

  `module-attribute`

```
SPECS_GSON = {'2158': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testByteSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testShortSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testIntSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testLongSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testFloatSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testDoubleSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testPrimitiveIntegerAutoboxedSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testPrimitiveIntegerAutoboxedInASingleElementArraySerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testReallyLongValuesSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testPrimitiveLongAutoboxedSerialization']}, '2024': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.functional.FieldNamingTest#testUpperCaseWithUnderscores', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.NamingPolicyTest#testGsonWithUpperCaseUnderscorePolicySerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.NamingPolicyTest#testGsonWithUpperCaseUnderscorePolicyDeserialiation']}, '2479': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.GsonBuilderTest#testRegisterTypeAdapterForObjectAndJsonElements', 'mvnd test -B -pl gson -Dtest=com.google.gson.GsonBuilderTest#testRegisterTypeHierarchyAdapterJsonElements', 'mvnd test -B -pl gson -Dtest=com.google.gson.GsonBuilderTest#testModificationAfterCreate']}, '2134': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.util.ISO8601UtilsTest#testDateParseInvalidDay', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.util.ISO8601UtilsTest#testDateParseInvalidMonth', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.util.ISO8601UtilsTest#testDateParseWithDefaultTimezone']}, '2061': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonReaderTest#testHasNextEndOfDocument', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testHasNext_endOfDocument', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonReaderTest#testReadEmptyObject', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonReaderTest#testReadEmptyArray', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testSkipValue_emptyJsonObject', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testSkipValue_filledJsonObject']}, '2311': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.JsonPrimitiveTest#testEqualsIntegerAndBigInteger', 'mvnd test -B -pl gson -Dtest=com.google.gson.JsonPrimitiveTest#testLongEqualsBigInteger', 'mvnd test -B -pl gson -Dtest=com.google.gson.JsonPrimitiveTest#testEqualsAcrossTypes']}, '1100': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.DefaultDateTypeAdapterTest#testNullValue', 'mvnd test -B -pl gson -Dtest=com.google.gson.DefaultDateTypeAdapterTest#testDatePattern', 'mvnd test -B -pl gson -Dtest=com.google.gson.DefaultDateTypeAdapterTest#testInvalidDatePattern']}, '1093': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testNonFiniteDoublesWhenLenient', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testNonFiniteBoxedDoublesWhenLenient', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testNonFiniteDoubles', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testNonFiniteBoxedDoubles', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testDoubles']}, '1014': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testSkipValue_emptyJsonObject', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testSkipValue_filledJsonObject']}}
```

#####
            SPECS_DRUID

  `module-attribute`

```
SPECS_DRUID = {'15402': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl processing -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl processing -Dtest=org.apache.druid.query.groupby.GroupByQueryQueryToolChestTest#testCacheStrategy', 'mvnd test -B -pl processing -Dtest=org.apache.druid.query.groupby.GroupByQueryQueryToolChestTest#testResultLevelCacheKeyWithSubTotalsSpec', 'mvnd test -B -pl processing -Dtest=org.apache.druid.query.groupby.GroupByQueryQueryToolChestTest#testMultiColumnCacheStrategy']}, '14092': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl processing,cloud/aws-common,cloud/gcp-common -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl server -Dtest=org.apache.druid.discovery.DruidLeaderClientTest#test503ResponseFromServerAndCacheRefresh', 'mvnd test -B -pl server -Dtest=org.apache.druid.discovery.DruidLeaderClientTest#testServerFailureAndRedirect']}, '14136': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl processing -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirstZeroLengthInterval', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirstZeroLengthInterval2', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirstZeroLengthInterval3', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirstZeroLengthInterval4', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapFirstContainsSecond', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirst']}, '13704': {'docker_specs': {'java_version': '11'}, 'install': ["sed -i 's/<resourceBundle>org.apache.apache.resources:apache-jar-resource-bundle:1.5-SNAPSHOT<\\/resourceBundle>/<resourceBundle>org.apache.apache.resources:apache-jar-resource-bundle:1.5<\\/resourceBundle>/' pom.xml", 'mvn clean install -B -pl processing -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl processing -Dtest=org.apache.druid.query.aggregation.post.ArithmeticPostAggregatorTest#testPow', 'mvnd test -B -pl processing -Dtest=org.apache.druid.query.aggregation.post.ArithmeticPostAggregatorTest#testDiv', 'mvnd test -B -pl processing -Dtest=org.apache.druid.query.aggregation.post.ArithmeticPostAggregatorTest#testQuotient']}, '16875': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl server -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl server -Dtest=org.apache.druid.server.metrics.WorkerTaskCountStatsMonitorTest#testMonitorWithPeon', 'mvnd test -B -pl server -Dtest=org.apache.druid.server.metrics.WorkerTaskCountStatsMonitorTest#testMonitorWithNulls', 'mvnd test -B -pl server -Dtest=org.apache.druid.server.metrics.WorkerTaskCountStatsMonitorTest#testMonitorIndexer']}}
```

#####
            SPECS_JAVAPARSER

  `module-attribute`

```
SPECS_JAVAPARSER = {'4561': {'docker_specs': {'java_version': '17'}, 'build': ['./mvnw clean install -B -pl javaparser-symbol-solver-testing -DskipTests -am'], 'test_cmd': ['./mvnw test -B -pl javaparser-symbol-solver-testing -Dtest=Issue4560Test', './mvnw test -B -pl javaparser-symbol-solver-testing -Dtest=JavaSymbolSolverTest']}, '4538': {'docker_specs': {'java_version': '17'}, 'build': ['./mvnw clean install -B -pl javaparser-core-testing -DskipTests -am'], 'test_cmd': ['./mvnw test -B -pl javaparser-core-testing -Dtest=NodeTest', './mvnw test -B -pl javaparser-core-testing -Dtest=NodePositionTest']}}
```

#####
            SPECS_LOMBOK

  `module-attribute`

```
SPECS_LOMBOK = {'3602': {'docker_specs': {'java_version': '11'}, 'pre_install': make_lombok_pre_install_script(['lombok.bytecode.TestPostCompiler']), 'build': ['ant test.compile'], 'test_cmd': ['ant test.instance']}, None: {k: {'docker_specs': {'java_version': '11'}, 'pre_install': make_lombok_pre_install_script(['lombok.transform.TestWithDelombok']), 'build': ['ant test.compile'], 'test_cmd': ['ant test.instance']} for k in ['3312', '3697', '3326', '3674', '3594', '3422', '3215', '3486', '3042', '3052', '2792']}, None: {k: {'docker_specs': {'java_version': '17'}, 'pre_install': make_lombok_pre_install_script(['lombok.transform.TestWithDelombok']), 'build': ['ant test.compile'], 'test_cmd': ['ant test.instance']} for k in ['3571', '3479', '3371', '3350', '3009']}}
```

#####
            SPECS_LUCENE

  `module-attribute`

```
SPECS_LUCENE = {'13494': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.facet.TestStringValueFacetCounts']}, '13704': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.search.TestLatLonDocValuesQueries']}, '13301': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests TestXYPoint.testEqualsAndHashCode -Dtests.seed=3ABEFE4D876DD310 -Dtests.nightly=true -Dtests.locale=es-419 -Dtests.timezone=Asia/Ulaanbaatar -Dtests.asserts=true -Dtests.file.encoding=UTF-8']}, '12626': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.index.TestIndexWriter']}, '12212': {'docker_specs': {'java_version': '17'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.facet.TestDrillSideways']}, '13170': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.analysis.opennlp.TestOpenNLPSentenceBreakIterator -Ptests.useSecurityManager=false']}, '12196': {'docker_specs': {'java_version': '17'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.queryparser.classic.TestMultiFieldQueryParser']}, '12022': {'docker_specs': {'java_version': '17'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.document.TestLatLonShape']}, '11760': {'docker_specs': {'java_version': '17'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.queries.intervals.TestIntervalBuilder']}}
```

#####
            SPECS_RXJAVA

  `module-attribute`

```
SPECS_RXJAVA = {'7597': {'docker_specs': {'java_version': '11'}, 'pre_install': make_rxjava_pre_install_script(), 'test_cmd': ['./gradlew test --tests io.reactivex.rxjava3.internal.operators.observable.ObservableSwitchTest']}}
```

#####
            MAP_REPO_VERSION_TO_SPECS_JAVA

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_JAVA = {'google/gson': SPECS_GSON, 'apache/druid': SPECS_DRUID, 'javaparser/javaparser': SPECS_JAVAPARSER, 'projectlombok/lombok': SPECS_LOMBOK, 'apache/lucene': SPECS_LUCENE, 'reactivex/rxjava': SPECS_RXJAVA}
```

#####
            MAP_REPO_TO_INSTALL_JAVA

  `module-attribute`

```
MAP_REPO_TO_INSTALL_JAVA = {}
```

#####
            TEST_XVFB_PREFIX

  `module-attribute`

```
TEST_XVFB_PREFIX = 'xvfb-run --server-args="-screen 0 1280x1024x24 -ac :99"'
```

#####
            XVFB_DEPS

  `module-attribute`

```
XVFB_DEPS = ['python3', 'python3-pip', 'xvfb', 'x11-xkb-utils', 'xfonts-100dpi', 'xfonts-75dpi', 'xfonts-scalable', 'xfonts-cyrillic', 'x11-apps', 'firefox']
```

#####
            X11_DEPS

  `module-attribute`

```
X11_DEPS = ['libx11-xcb1', 'libxcomposite1', 'libxcursor1', 'libxdamage1', 'libxi6', 'libxtst6', 'libnss3', 'libcups2', 'libxss1', 'libxrandr2', 'libasound2', 'libatk1.0-0', 'libgtk-3-0', 'x11-utils']
```

#####
            SPECS_CALYPSO

  `module-attribute`

```
SPECS_CALYPSO = {None: {k: {'apt-pkgs': ['libsass-dev', 'sassc'], 'install': ['npm install --unsafe-perm'], 'test_cmd': 'npm run test-client', 'docker_specs': {'node_version': k}} for k in ['0.8', '4.2.3', '4.3.0', '5.10.1', '5.11.1', '6.1.0', '6.7.0', '6.9.0', '6.9.1', '6.9.4', '6.10.0', '6.10.2', '6.10.3', '6.11.1', '6.11.2', '6.11.5', '8.9.1', '8.9.3', '8.9.4', '8.11.0', '8.11.2', '10.4.1', '10.5.0', '10.6.0', '10.9.0', '10.10.0', '10.12.0', '10.13.0', '10.14.0', '10.15.2', '10.16.3']}}
```

#####
            TEST_CHART_JS_TEMPLATE

  `module-attribute`

```
TEST_CHART_JS_TEMPLATE = './node_modules/.bin/cross-env NODE_ENV=test ./node_modules/.bin/karma start {} --single-run --coverage --grep --auto-watch false'
```

#####
            SPECS_CHART_JS

  `module-attribute`

```
SPECS_CHART_JS = {None: {k: {'install': ['pnpm install', 'pnpm run build'], 'test_cmd': ['pnpm install', 'pnpm run build', f'{TEST_XVFB_PREFIX} su chromeuser -c "{format('./karma.conf.cjs')}"'], 'docker_specs': {'node_version': '21.6.2', 'pnpm_version': '7.9.0', 'run_args': {'cap_add': ['SYS_ADMIN']}}} for k in ['4.0', '4.1', '4.2', '4.3', '4.4']}, None: {k: {'install': ['npm install'], 'test_cmd': ['npm install', 'npm run build', f'{TEST_XVFB_PREFIX} su chromeuser -c "{format('./karma.conf.js')}"'], 'docker_specs': {'node_version': '21.6.2', 'run_args': {'cap_add': ['SYS_ADMIN']}}} for k in ['3.0', '3.1', '3.2', '3.3', '3.4', '3.5', '3.6', '3.7', '3.8']}, None: {k: {'install': ['npm install', 'npm install -g gulp-cli'], 'test_cmd': ['npm install', 'gulp build', TEST_XVFB_PREFIX + ' su chromeuser -c "gulp test"'], 'docker_specs': {'node_version': '21.6.2', 'run_args': {'cap_add': ['SYS_ADMIN']}}} for k in ['2.0', '2.1', '2.2', '2.3', '2.4', '2.5', '2.6', '2.7', '2.8', '2.9']}}
```

#####
            SPECS_MARKED

  `module-attribute`

```
SPECS_MARKED = {None: {k: {'install': ['npm install'], 'test_cmd': './node_modules/.bin/jasmine --no-color --config=jasmine.json', 'docker_specs': {'node_version': '12.22.12'}} for k in ['0.3', '0.5', '0.6', '0.7', '1.0', '1.1', '1.2', '2.0', '3.9', '4.0', '4.1', '5.0']}}
```

#####
            SPECS_P5_JS

  `module-attribute`

```
SPECS_P5_JS = {None: {k: {'apt-pkgs': X11_DEPS, 'install': ['npm install', "PUPPETEER_SKIP_CHROMIUM_DOWNLOAD='' node node_modules/puppeteer/install.js", './node_modules/.bin/grunt yui'], 'test_cmd': "sed -i 's/concurrency:[[:space:]]*[0-9][0-9]*/concurrency: 1/g' Gruntfile.js\nstdbuf -o 1M ./node_modules/.bin/grunt test --quiet --force", 'docker_specs': {'node_version': '14.17.3'}} for k in ['0.10', '0.2', '0.4', '0.5', '0.6', '0.7', '0.8', '0.9', '1.0', '1.1', '1.2', '1.3', '1.4', '1.5', '1.6', '1.7', '1.8', '1.9']}}
```

#####
            SPECS_REACT_PDF

  `module-attribute`

```
SPECS_REACT_PDF = {None: {k: {'apt-pkgs': ['pkg-config', 'build-essential', 'libpixman-1-0', 'libpixman-1-dev', 'libcairo2-dev', 'libpango1.0-dev', 'libjpeg-dev', 'libgif-dev', 'librsvg2-dev'] + X11_DEPS, 'install': ['npm i -g yarn', 'yarn install'], 'test_cmd': 'NODE_OPTIONS="--experimental-vm-modules" ./node_modules/.bin/jest --no-color', 'docker_specs': {'node_version': '18.20.4'}} for k in ['1.0', '1.1', '1.2', '2.0']}}
```

#####
            JEST_JSON_JQ_TRANSFORM

  `module-attribute`

```
JEST_JSON_JQ_TRANSFORM = 'jq -r \'.testResults[].assertionResults[] | "[" + (.status | ascii_upcase) + "] " + ((.ancestorTitles | join(" > ")) + (if .ancestorTitles | length > 0 then " > " else "" end) + .title)\''
```

#####
            SPECS_BABEL

  `module-attribute`

```
SPECS_BABEL = {'14532': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest babel-generator --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}, '13928': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest babel-parser -t "arrow" --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}, '15649': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest packages/babel-traverse/test/scope.js --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}, '15445': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest packages/babel-generator/test/index.js -t "generation " --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}, '16130': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest babel-helpers --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}}
```

#####
            SPECS_VUEJS

  `module-attribute`

```
SPECS_VUEJS = {'11899': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/compiler-sfc/__tests__/compileStyle.spec.ts --no-watch --reporter=verbose'], 'install': ['pnpm i'], 'build': ['pnpm run build compiler-sfc']}, '11870': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/runtime-core/__tests__/helpers/renderList.spec.ts --no-watch --reporter=verbose'], 'install': ['pnpm i']}, '11739': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/runtime-core/__tests__/hydration.spec.ts --no-watch --reporter=verbose -t "mismatch handling"'], 'install': ['pnpm i']}, '11915': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/compiler-core/__tests__/parse.spec.ts --no-watch --reporter=verbose -t "Element"'], 'install': ['pnpm i']}, '11589': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/runtime-core/__tests__/apiWatch.spec.ts --no-watch --reporter=verbose'], 'install': ['pnpm i']}}
```

#####
            SPECS_DOCUSAURUS

  `module-attribute`

```
SPECS_DOCUSAURUS = {'10309': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus-plugin-content-docs/src/client/__tests__/docsClientUtils.test.ts --verbose']}, '10130': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus/src/server/__tests__/brokenLinks.test.ts --verbose']}, '9897': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus-utils/src/__tests__/markdownUtils.test.ts --verbose']}, '9183': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus-theme-classic/src/__tests__/options.test.ts --verbose']}, '8927': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus-utils/src/__tests__/markdownLinks.test.ts --verbose']}}
```

#####
            SPECS_IMMUTABLEJS

  `module-attribute`

```
SPECS_IMMUTABLEJS = {'2006': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'build': ['npm run build'], 'test_cmd': ['npx jest __tests__/Range.ts --verbose']}, '2005': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'build': ['npm run build'], 'test_cmd': [f'npx jest __tests__/OrderedMap.ts __tests__/OrderedSet.ts --silent --json | {JEST_JSON_JQ_TRANSFORM}']}}
```

#####
            SPECS_THREEJS

  `module-attribute`

```
SPECS_THREEJS = {'27395': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install --ignore-scripts'], 'test_cmd': ['npx qunit test/unit/src/math/Sphere.tests.js']}, '26589': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install --ignore-scripts'], 'test_cmd': ['npx qunit test/unit/src/objects/Line.tests.js test/unit/src/objects/Mesh.tests.js test/unit/src/objects/Points.tests.js']}, '25687': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install --ignore-scripts'], 'test_cmd': ['npx qunit test/unit/src/core/Object3D.tests.js -f "/json|clone|copy/i"']}}
```

#####
            SPECS_PREACT

  `module-attribute`

```
SPECS_PREACT = {'4152': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/components.test.js"']}, '4316': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/events.test.js"']}, '4245': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/useId.test.js"']}, '4182': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/errorBoundary.test.js"']}, '4436': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/refs.test.js"']}, '3763': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/lifecycles/componentDidMount.test.js"']}, '3739': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/useState.test.js"']}, '3689': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/errorBoundary.test.js"']}, '3567': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/useEffect.test.js"']}, '3562': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="compat/test/browser/render.test.js"']}, '3454': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/svg.test.js"']}, '3345': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/useEffect.test.js"']}, '3062': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/render.test.js"']}, '3010': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/render.test.js"']}, '2927': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/render.test.js"']}, '2896': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="compat/test/browser/memo.test.js"']}, '2757': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/render.test.js"']}}
```

#####
            SPECS_AXIOS

  `module-attribute`

```
SPECS_AXIOS = {'5892': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ["npx mocha test/unit/adapters/http.js -R tap -g 'compression'"]}, '5316': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'build': ['npm install'], 'test_cmd': ["npx mocha test/unit/adapters/http.js -R tap -g 'FormData'"]}, '4738': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ["timeout 10s npx mocha -R tap test/unit/adapters/http.js -g 'timeout'"]}, '4731': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ["npx mocha -R tap test/unit/adapters/http.js -g 'body length'"]}, '6539': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['npx mocha -R tap test/unit/regression/SNYK-JS-AXIOS-7361793.js']}, '5085': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['npx mocha -R tap test/unit/regression/bugs.js']}}
```

#####
            MAP_REPO_VERSION_TO_SPECS_JS

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_JS = {'Automattic/wp-calypso': SPECS_CALYPSO, 'chartjs/Chart.js': SPECS_CHART_JS, 'markedjs/marked': SPECS_MARKED, 'processing/p5.js': SPECS_P5_JS, 'diegomura/react-pdf': SPECS_REACT_PDF, 'babel/babel': SPECS_BABEL, 'vuejs/core': SPECS_VUEJS, 'facebook/docusaurus': SPECS_DOCUSAURUS, 'immutable-js/immutable-js': SPECS_IMMUTABLEJS, 'mrdoob/three.js': SPECS_THREEJS, 'preactjs/preact': SPECS_PREACT, 'axios/axios': SPECS_AXIOS}
```

#####
            MAP_REPO_TO_INSTALL_JS

  `module-attribute`

```
MAP_REPO_TO_INSTALL_JS = {}
```

#####
            SPECS_PHPSPREADSHEET

  `module-attribute`

```
SPECS_PHPSPREADSHEET = {'4313': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Reader/Ods/FormulaTranslatorTest.php']}, '4214': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Calculation/Functions/MathTrig/RoundDownTest.php']}, '4186': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Writer/Xlsx/FunctionPrefixTest.php']}, '4114': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Worksheet/Issue4112Test.php']}, '3940': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Worksheet/WorksheetTest.php']}, '3903': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Shared/StringHelperTest.php']}, '3570': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Calculation/Functions/LookupRef/VLookupTest.php']}, '3463': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Writer/Xlsx/FunctionPrefixTest.php']}, '3469': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Style/StyleTest.php']}, '3659': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Worksheet/Table/Issue3635Test.php']}}
```

#####
            SPECS_LARAVEL_FRAMEWORK

  `module-attribute`

```
SPECS_LARAVEL_FRAMEWORK = {'53914': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Integration/Database/DatabaseConnectionsTest.php']}, '53206': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Support/SupportJsTest.php']}, '52866': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Container/ContextualAttributeBindingTest.php']}, '52684': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Support/SupportStrTest.php']}, '52680': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Database/DatabaseEloquentInverseRelationTest.php']}, '52451': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ["vendor/bin/phpunit --testdox --colors=never tests/Validation/ValidationValidatorTest.php --filter 'custom'"]}, '53949': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Support/OnceTest.php']}, '51890': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ["vendor/bin/phpunit --testdox --colors=never tests/Validation/ValidationValidatorTest.php --filter 'attribute'"]}, '51195': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/View/Blade/BladeVerbatimTest.php']}, '48636': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Database/DatabaseEloquentModelTest.php']}, '48573': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Cache/CacheArrayStoreTest.php']}, '46234': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Routing/RoutingUrlGeneratorTest.php']}, '53696': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Database/DatabaseSchemaBlueprintTest.php']}}
```

#####
            SPECS_PHP_CS_FIXER

  `module-attribute`

```
SPECS_PHP_CS_FIXER = {'8367': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Import/FullyQualifiedStrictTypesFixerTest.php']}, '8331': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/LanguageConstruct/NullableTypeDeclarationFixerTest.php']}, '8075': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/PhpUnit/PhpUnitAttributesFixerTest.php']}, '8064': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/StringNotation/SimpleToComplexStringVariableFixerTest.php']}, '7998': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Casing/ConstantCaseFixerTest.php']}, '7875': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Whitespace/StatementIndentationFixerTest.php']}, '7635': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Import/FullyQualifiedStrictTypesFixerTest.php']}, '7523': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Operator/BinaryOperatorSpacesFixerTest.php']}, '8256': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/PhpTag/BlankLineAfterOpeningTagFixerTest.php']}, '7663': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Whitespace/StatementIndentationFixerTest.php']}}
```

#####
            SPECS_CARBON

  `module-attribute`

```
SPECS_CARBON = {'3103': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonImmutable/SettersTest.php']}, '3098': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/ConstructTest.php']}, '3073': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/TotalTest.php']}, '3041': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonPeriod/CreateTest.php']}, '3005': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/ConstructTest.php']}, '2981': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/TotalTest.php']}, '2813': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'build': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Factory/FactoryTest.php']}, '2752': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonImmutable/IsTest.php']}, '2665': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Carbon/RoundTest.php']}, '2762': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/RoundingTest.php']}}
```

#####
            MAP_REPO_VERSION_TO_SPECS_PHP

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_PHP = {'phpoffice/phpspreadsheet': SPECS_PHPSPREADSHEET, 'laravel/framework': SPECS_LARAVEL_FRAMEWORK, 'php-cs-fixer/php-cs-fixer': SPECS_PHP_CS_FIXER, 'briannesbitt/carbon': SPECS_CARBON}
```

#####
            MAP_REPO_TO_INSTALL_PHP

  `module-attribute`

```
MAP_REPO_TO_INSTALL_PHP = {}
```

#####
            TEST_PYTEST

  `module-attribute`

```
TEST_PYTEST = 'pytest -rA'
```

#####
            TEST_PYTEST_VERBOSE

  `module-attribute`

```
TEST_PYTEST_VERBOSE = 'pytest -rA --tb=long'
```

#####
            TEST_ASTROPY_PYTEST

  `module-attribute`

```
TEST_ASTROPY_PYTEST = 'pytest -rA -vv -o console_output_style=classic --tb=no'
```

#####
            TEST_DJANGO

  `module-attribute`

```
TEST_DJANGO = './tests/runtests.py --verbosity 2 --settings=test_sqlite --parallel 1'
```

#####
            TEST_DJANGO_NO_PARALLEL

  `module-attribute`

```
TEST_DJANGO_NO_PARALLEL = './tests/runtests.py --verbosity 2'
```

#####
            TEST_SEABORN

  `module-attribute`

```
TEST_SEABORN = 'pytest --no-header -rA'
```

#####
            TEST_SEABORN_VERBOSE

  `module-attribute`

```
TEST_SEABORN_VERBOSE = 'pytest -rA --tb=long'
```

#####
            TEST_SPHINX

  `module-attribute`

```
TEST_SPHINX = 'tox --current-env -epy39 -v --'
```

#####
            TEST_SYMPY

  `module-attribute`

```
TEST_SYMPY = "PYTHONWARNINGS='ignore::UserWarning,ignore::SyntaxWarning' bin/test -C --verbose"
```

#####
            TEST_SYMPY_VERBOSE

  `module-attribute`

```
TEST_SYMPY_VERBOSE = 'bin/test -C --verbose'
```

#####
            SPECS_SKLEARN

  `module-attribute`

```
SPECS_SKLEARN = {k: {'python': '3.6', 'packages': 'numpy scipy cython pytest pandas matplotlib', 'install': 'python -m pip install -v --no-use-pep517 --no-build-isolation -e .', 'pip_packages': ['cython', 'numpy==1.19.2', 'setuptools', 'scipy==1.5.2'], 'test_cmd': TEST_PYTEST} for k in ['0.20', '0.21', '0.22']}
```

#####
            SPECS_FLASK

  `module-attribute`

```
SPECS_FLASK = {'2.0': {'python': '3.9', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .', 'pip_packages': ['setuptools==70.0.0', 'Werkzeug==2.3.7', 'Jinja2==3.0.1', 'itsdangerous==2.1.2', 'click==8.0.1', 'MarkupSafe==2.1.3'], 'test_cmd': TEST_PYTEST}, '2.1': {'python': '3.10', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .', 'pip_packages': ['setuptools==70.0.0', 'click==8.1.3', 'itsdangerous==2.1.2', 'Jinja2==3.1.2', 'MarkupSafe==2.1.1', 'Werkzeug==2.3.7'], 'test_cmd': TEST_PYTEST}}
```

#####
            SPECS_DJANGO

  `module-attribute`

```
SPECS_DJANGO = {k: {'python': '3.5', 'packages': 'requirements.txt', 'pre_install': ['apt-get update && apt-get install -y locales', "echo 'en_US UTF-8' > /etc/locale.gen", 'locale-gen en_US.UTF-8'], 'install': 'python setup.py install', 'pip_packages': ['setuptools'], 'eval_commands': ['export LANG=en_US.UTF-8', 'export LC_ALL=en_US.UTF-8', 'export PYTHONIOENCODING=utf8', 'export LANGUAGE=en_US:en'], 'test_cmd': TEST_DJANGO} for k in ['1.7', '1.8', '1.9', '1.10', '1.11', '2.0', '2.1', '2.2']}
```

#####
            SPECS_REQUESTS

  `module-attribute`

```
SPECS_REQUESTS = {k: {'python': '3.9', 'packages': 'pytest', 'install': 'python -m pip install .', 'test_cmd': TEST_PYTEST} for k in (['0.7', '0.8', '0.9', '0.11', '0.13', '0.14', '1.1', '1.2', '2.0', '2.2'] + ['2.3', '2.4', '2.5', '2.7', '2.8', '2.9', '2.10', '2.11', '2.12', '2.17'] + ['2.18', '2.19', '2.22', '2.26', '2.25', '2.27', '2.31', '3.0'])}
```

#####
            SPECS_SEABORN

  `module-attribute`

```
SPECS_SEABORN = {k: {'python': '3.9', 'install': 'python -m pip install -e .', 'pip_packages': ['contourpy==1.1.0', 'cycler==0.11.0', 'fonttools==4.42.1', 'importlib-resources==6.0.1', 'kiwisolver==1.4.5', 'matplotlib==3.7.2', 'numpy==1.25.2', 'packaging==23.1', 'pandas==1.3.5', 'pillow==10.0.0', 'pyparsing==3.0.9', 'pytest', 'python-dateutil==2.8.2', 'pytz==2023.3.post1', 'scipy==1.11.2', 'six==1.16.0', 'tzdata==2023.1', 'zipp==3.16.2'], 'test_cmd': TEST_SEABORN} for k in ['0.11']}
```

#####
            SPECS_PYTEST

  `module-attribute`

```
SPECS_PYTEST = {k: {'python': '3.9', 'install': 'python -m pip install -e .', 'test_cmd': TEST_PYTEST} for k in ['4.4', '4.5', '4.6', '5.0', '5.1', '5.2', '5.3', '5.4', '6.0', '6.2', '6.3', '7.0', '7.1', '7.2', '7.4', '8.0', '8.1', '8.2', '8.3', '8.4']}
```

#####
            SPECS_MATPLOTLIB

  `module-attribute`

```
SPECS_MATPLOTLIB = {k: {'python': '3.11', 'packages': 'environment.yml', 'install': 'python -m pip install -e .', 'pre_install': ['apt-get -y update && apt-get -y upgrade && DEBIAN_FRONTEND=noninteractive apt-get install -y imagemagick ffmpeg texlive texlive-latex-extra texlive-fonts-recommended texlive-xetex texlive-luatex cm-super dvipng', 'QHULL_URL="http://www.qhull.org/download/qhull-2020-src-8.0.2.tgz"', 'QHULL_TAR="/tmp/qhull-2020-src-8.0.2.tgz"', 'QHULL_BUILD_DIR="/testbed/build"', 'wget -O "$QHULL_TAR" "$QHULL_URL"', 'mkdir -p "$QHULL_BUILD_DIR"', 'tar -xvzf "$QHULL_TAR" -C "$QHULL_BUILD_DIR"'], 'pip_packages': ['contourpy==1.1.0', 'cycler==0.11.0', 'fonttools==4.42.1', 'ghostscript', 'kiwisolver==1.4.5', 'numpy==1.25.2', 'packaging==23.1', 'pillow==10.0.0', 'pikepdf', 'pyparsing==3.0.9', 'python-dateutil==2.8.2', 'six==1.16.0', 'setuptools==68.1.2', 'setuptools-scm==7.1.0', 'typing-extensions==4.7.1'], 'test_cmd': TEST_PYTEST} for k in ['3.5', '3.6', '3.7', '3.8', '3.9']}
```

#####
            SPECS_SPHINX

  `module-attribute`

```
SPECS_SPHINX = {k: {'python': '3.9', 'pip_packages': ['tox==4.16.0', 'tox-current-env==0.0.11', 'Jinja2==3.0.3'], 'install': 'python -m pip install -e .[test]', 'pre_install': ["sed -i 's/pytest/pytest -rA/' tox.ini"], 'test_cmd': TEST_SPHINX} for k in (['1.5', '1.6', '1.7', '1.8', '2.0', '2.1', '2.2', '2.3', '2.4', '3.0'] + ['3.1', '3.2', '3.3', '3.4', '3.5', '4.0', '4.1', '4.2', '4.3', '4.4'] + ['4.5', '5.0', '5.1', '5.2', '5.3', '6.0', '6.2', '7.0', '7.1', '7.2'] + ['7.3', '7.4', '8.0', '8.1'])}
```

#####
            SPECS_ASTROPY

  `module-attribute`

```
SPECS_ASTROPY = {k: {'python': '3.9', 'install': 'python -m pip install -e .[test] --verbose', 'pip_packages': ['attrs==23.1.0', 'exceptiongroup==1.1.3', 'execnet==2.0.2', 'hypothesis==6.82.6', 'iniconfig==2.0.0', 'numpy==1.25.2', 'packaging==23.1', 'pluggy==1.3.0', 'psutil==5.9.5', 'pyerfa==2.0.0.3', 'pytest-arraydiff==0.5.0', 'pytest-astropy-header==0.2.2', 'pytest-astropy==0.10.0', 'pytest-cov==4.1.0', 'pytest-doctestplus==1.0.0', 'pytest-filter-subpackage==0.1.2', 'pytest-mock==3.11.1', 'pytest-openfiles==0.5.0', 'pytest-remotedata==0.4.0', 'pytest-xdist==3.3.1', 'pytest==7.4.0', 'PyYAML==6.0.1', 'setuptools==68.0.0', 'sortedcontainers==2.4.0', 'tomli==2.0.1'], 'test_cmd': TEST_PYTEST} for k in ['3.0', '3.1', '3.2', '4.1', '4.2', '4.3', '5.0', '5.1', '5.2', 'v5.3']}
```

#####
            SPECS_SYMPY

  `module-attribute`

```
SPECS_SYMPY = {k: {'python': '3.9', 'packages': 'mpmath flake8', 'pip_packages': ['mpmath==1.3.0', 'flake8-comprehensions'], 'install': 'python -m pip install -e .', 'test_cmd': TEST_SYMPY} for k in (['0.7', '1.0', '1.1', '1.10', '1.11', '1.12', '1.2', '1.4', '1.5', '1.6'] + ['1.7', '1.8', '1.9'] + ['1.10', '1.11', '1.12', '1.13', '1.14'])}
```

#####
            SPECS_PYLINT

  `module-attribute`

```
SPECS_PYLINT = {k: {'python': '3.9', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .', 'test_cmd': TEST_PYTEST} for k in ['2.10', '2.11', '2.13', '2.14', '2.15', '2.16', '2.17', '2.8', '2.9', '3.0', '3.1', '3.2', '3.3', '4.0']}
```

#####
            SPECS_XARRAY

  `module-attribute`

```
SPECS_XARRAY = {k: {'python': '3.10', 'packages': 'environment.yml', 'install': 'python -m pip install -e .', 'pip_packages': ['numpy==1.23.0', 'packaging==23.1', 'pandas==1.5.3', 'pytest==7.4.0', 'python-dateutil==2.8.2', 'pytz==2023.3', 'six==1.16.0', 'scipy==1.11.1', 'setuptools==68.0.0', 'dask==2022.8.1'], 'no_use_env': True, 'test_cmd': TEST_PYTEST} for k in ['0.12', '0.18', '0.19', '0.20', '2022.03', '2022.06', '2022.09', '2023.07', '2024.05']}
```

#####
            SPECS_SQLFLUFF

  `module-attribute`

```
SPECS_SQLFLUFF = {k: {'python': '3.9', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .', 'test_cmd': TEST_PYTEST} for k in ['0.10', '0.11', '0.12', '0.13', '0.4', '0.5', '0.6', '0.8', '0.9', '1.0', '1.1', '1.2', '1.3', '1.4', '2.0', '2.1', '2.2']}
```

#####
            SPECS_DBT_CORE

  `module-attribute`

```
SPECS_DBT_CORE = {k: {'python': '3.9', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .'} for k in ['0.13', '0.14', '0.15', '0.16', '0.17', '0.18', '0.19', '0.20', '0.21', '1.0', '1.1', '1.2', '1.3', '1.4', '1.5', '1.6', '1.7']}
```

#####
            SPECS_PYVISTA

  `module-attribute`

```
SPECS_PYVISTA = {k: {'python': '3.9', 'install': 'python -m pip install -e .', 'pip_packages': ['pytest'], 'test_cmd': TEST_PYTEST} for k in ['0.20', '0.21', '0.22', '0.23']}
```

#####
            SPECS_ASTROID

  `module-attribute`

```
SPECS_ASTROID = {k: {'python': '3.9', 'install': 'python -m pip install -e .', 'pip_packages': ['pytest'], 'test_cmd': TEST_PYTEST} for k in ['2.10', '2.12', '2.13', '2.14', '2.15', '2.16', '2.5', '2.6', '2.7', '2.8', '2.9', '3.0']}
```

#####
            SPECS_MARSHMALLOW

  `module-attribute`

```
SPECS_MARSHMALLOW = {k: {'python': '3.9', 'install': "python -m pip install -e '.[dev]'", 'test_cmd': TEST_PYTEST} for k in ['2.18', '2.19', '2.20', '3.0', '3.1', '3.10', '3.11', '3.12', '3.13', '3.15', '3.16', '3.19', '3.2', '3.4', '3.8', '3.9']}
```

#####
            SPECS_PVLIB

  `module-attribute`

```
SPECS_PVLIB = {k: {'python': '3.9', 'install': 'python -m pip install -e .[all]', 'packages': 'pandas scipy', 'pip_packages': ['jupyter', 'ipython', 'matplotlib', 'pytest', 'flake8'], 'test_cmd': TEST_PYTEST} for k in ['0.1', '0.2', '0.3', '0.4', '0.5', '0.6', '0.7', '0.8', '0.9']}
```

#####
            SPECS_PYDICOM

  `module-attribute`

```
SPECS_PYDICOM = {k: {'python': '3.6', 'install': 'python -m pip install -e .', 'packages': 'numpy', 'pip_packages': ['pytest'], 'test_cmd': TEST_PYTEST} for k in ['1.0', '1.1', '1.2', '1.3', '1.4', '2.0', '2.1', '2.2', '2.3', '2.4', '3.0']}
```

#####
            SPECS_HUMANEVAL

  `module-attribute`

```
SPECS_HUMANEVAL = {k: {'python': '3.9', 'test_cmd': 'python'} for k in ['1.0']}
```

#####
            MAP_REPO_VERSION_TO_SPECS_PY

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_PY = {'astropy/astropy': SPECS_ASTROPY, 'dbt-labs/dbt-core': SPECS_DBT_CORE, 'django/django': SPECS_DJANGO, 'matplotlib/matplotlib': SPECS_MATPLOTLIB, 'marshmallow-code/marshmallow': SPECS_MARSHMALLOW, 'mwaskom/seaborn': SPECS_SEABORN, 'pallets/flask': SPECS_FLASK, 'psf/requests': SPECS_REQUESTS, 'pvlib/pvlib-python': SPECS_PVLIB, 'pydata/xarray': SPECS_XARRAY, 'pydicom/pydicom': SPECS_PYDICOM, 'pylint-dev/astroid': SPECS_ASTROID, 'pylint-dev/pylint': SPECS_PYLINT, 'pytest-dev/pytest': SPECS_PYTEST, 'pyvista/pyvista': SPECS_PYVISTA, 'scikit-learn/scikit-learn': SPECS_SKLEARN, 'sphinx-doc/sphinx': SPECS_SPHINX, 'sqlfluff/sqlfluff': SPECS_SQLFLUFF, 'swe-bench/humaneval': SPECS_HUMANEVAL, 'sympy/sympy': SPECS_SYMPY}
```

#####
            MAP_REPO_TO_INSTALL_PY

  `module-attribute`

```
MAP_REPO_TO_INSTALL_PY = {}
```

#####
            MAP_REPO_TO_REQS_PATHS

  `module-attribute`

```
MAP_REPO_TO_REQS_PATHS = {'dbt-labs/dbt-core': ['dev-requirements.txt', 'dev_requirements.txt'], 'django/django': ['tests/requirements/py3.txt'], 'matplotlib/matplotlib': ['requirements/dev/dev-requirements.txt', 'requirements/testing/travis_all.txt'], 'pallets/flask': ['requirements/dev.txt'], 'pylint-dev/pylint': ['requirements_test.txt'], 'pyvista/pyvista': ['requirements_test.txt', 'requirements.txt'], 'sqlfluff/sqlfluff': ['requirements_dev.txt'], 'sympy/sympy': ['requirements-dev.txt', 'requirements-test.txt']}
```

#####
            MAP_REPO_TO_ENV_YML_PATHS

  `module-attribute`

```
MAP_REPO_TO_ENV_YML_PATHS = {'matplotlib/matplotlib': ['environment.yml'], 'pydata/xarray': ['ci/requirements/environment.yml', 'environment.yml']}
```

#####
            USE_X86_PY

  `module-attribute`

```
USE_X86_PY = {'astropy__astropy-7973', 'django__django-10087', 'django__django-10097', 'django__django-10213', 'django__django-10301', 'django__django-10316', 'django__django-10426', 'django__django-11383', 'django__django-12185', 'django__django-12497', 'django__django-13121', 'django__django-13417', 'django__django-13431', 'django__django-13447', 'django__django-14155', 'django__django-14164', 'django__django-14169', 'django__django-14170', 'django__django-15180', 'django__django-15199', 'django__django-15280', 'django__django-15292', 'django__django-15474', 'django__django-15682', 'django__django-15689', 'django__django-15695', 'django__django-15698', 'django__django-15781', 'django__django-15925', 'django__django-15930', 'django__django-5158', 'django__django-5470', 'django__django-7188', 'django__django-7475', 'django__django-7530', 'django__django-8326', 'django__django-8961', 'django__django-9003', 'django__django-9703', 'django__django-9871', 'matplotlib__matplotlib-13983', 'matplotlib__matplotlib-13984', 'matplotlib__matplotlib-13989', 'matplotlib__matplotlib-14043', 'matplotlib__matplotlib-14471', 'matplotlib__matplotlib-22711', 'matplotlib__matplotlib-22719', 'matplotlib__matplotlib-22734', 'matplotlib__matplotlib-22767', 'matplotlib__matplotlib-22815', 'matplotlib__matplotlib-22835', 'matplotlib__matplotlib-22865', 'matplotlib__matplotlib-22871', 'matplotlib__matplotlib-22883', 'matplotlib__matplotlib-22926', 'matplotlib__matplotlib-22929', 'matplotlib__matplotlib-22931', 'matplotlib__matplotlib-22945', 'matplotlib__matplotlib-22991', 'matplotlib__matplotlib-23031', 'matplotlib__matplotlib-23047', 'matplotlib__matplotlib-23049', 'matplotlib__matplotlib-23057', 'matplotlib__matplotlib-23088', 'matplotlib__matplotlib-23111', 'matplotlib__matplotlib-23140', 'matplotlib__matplotlib-23174', 'matplotlib__matplotlib-23188', 'matplotlib__matplotlib-23198', 'matplotlib__matplotlib-23203', 'matplotlib__matplotlib-23266', 'matplotlib__matplotlib-23267', 'matplotlib__matplotlib-23288', 'matplotlib__matplotlib-23299', 'matplotlib__matplotlib-23314', 'matplotlib__matplotlib-23332', 'matplotlib__matplotlib-23348', 'matplotlib__matplotlib-23412', 'matplotlib__matplotlib-23476', 'matplotlib__matplotlib-23516', 'matplotlib__matplotlib-23562', 'matplotlib__matplotlib-23563', 'matplotlib__matplotlib-23573', 'matplotlib__matplotlib-23740', 'matplotlib__matplotlib-23742', 'matplotlib__matplotlib-23913', 'matplotlib__matplotlib-23964', 'matplotlib__matplotlib-23987', 'matplotlib__matplotlib-24013', 'matplotlib__matplotlib-24026', 'matplotlib__matplotlib-24088', 'matplotlib__matplotlib-24111', 'matplotlib__matplotlib-24149', 'matplotlib__matplotlib-24177', 'matplotlib__matplotlib-24189', 'matplotlib__matplotlib-24224', 'matplotlib__matplotlib-24250', 'matplotlib__matplotlib-24257', 'matplotlib__matplotlib-24265', 'matplotlib__matplotlib-24334', 'matplotlib__matplotlib-24362', 'matplotlib__matplotlib-24403', 'matplotlib__matplotlib-24431', 'matplotlib__matplotlib-24538', 'matplotlib__matplotlib-24570', 'matplotlib__matplotlib-24604', 'matplotlib__matplotlib-24619', 'matplotlib__matplotlib-24627', 'matplotlib__matplotlib-24637', 'matplotlib__matplotlib-24691', 'matplotlib__matplotlib-24749', 'matplotlib__matplotlib-24768', 'matplotlib__matplotlib-24849', 'matplotlib__matplotlib-24870', 'matplotlib__matplotlib-24912', 'matplotlib__matplotlib-24924', 'matplotlib__matplotlib-24970', 'matplotlib__matplotlib-24971', 'matplotlib__matplotlib-25027', 'matplotlib__matplotlib-25052', 'matplotlib__matplotlib-25079', 'matplotlib__matplotlib-25085', 'matplotlib__matplotlib-25122', 'matplotlib__matplotlib-25126', 'matplotlib__matplotlib-25129', 'matplotlib__matplotlib-25238', 'matplotlib__matplotlib-25281', 'matplotlib__matplotlib-25287', 'matplotlib__matplotlib-25311', 'matplotlib__matplotlib-25332', 'matplotlib__matplotlib-25334', 'matplotlib__matplotlib-25340', 'matplotlib__matplotlib-25346', 'matplotlib__matplotlib-25404', 'matplotlib__matplotlib-25405', 'matplotlib__matplotlib-25425', 'matplotlib__matplotlib-25430', 'matplotlib__matplotlib-25433', 'matplotlib__matplotlib-25442', 'matplotlib__matplotlib-25479', 'matplotlib__matplotlib-25498', 'matplotlib__matplotlib-25499', 'matplotlib__matplotlib-25515', 'matplotlib__matplotlib-25547', 'matplotlib__matplotlib-25551', 'matplotlib__matplotlib-25565', 'matplotlib__matplotlib-25624', 'matplotlib__matplotlib-25631', 'matplotlib__matplotlib-25640', 'matplotlib__matplotlib-25651', 'matplotlib__matplotlib-25667', 'matplotlib__matplotlib-25712', 'matplotlib__matplotlib-25746', 'matplotlib__matplotlib-25772', 'matplotlib__matplotlib-25775', 'matplotlib__matplotlib-25779', 'matplotlib__matplotlib-25785', 'matplotlib__matplotlib-25794', 'matplotlib__matplotlib-25859', 'matplotlib__matplotlib-25960', 'matplotlib__matplotlib-26011', 'matplotlib__matplotlib-26020', 'matplotlib__matplotlib-26024', 'matplotlib__matplotlib-26078', 'matplotlib__matplotlib-26089', 'matplotlib__matplotlib-26101', 'matplotlib__matplotlib-26113', 'matplotlib__matplotlib-26122', 'matplotlib__matplotlib-26160', 'matplotlib__matplotlib-26184', 'matplotlib__matplotlib-26208', 'matplotlib__matplotlib-26223', 'matplotlib__matplotlib-26232', 'matplotlib__matplotlib-26249', 'matplotlib__matplotlib-26278', 'matplotlib__matplotlib-26285', 'matplotlib__matplotlib-26291', 'matplotlib__matplotlib-26300', 'matplotlib__matplotlib-26311', 'matplotlib__matplotlib-26341', 'matplotlib__matplotlib-26342', 'matplotlib__matplotlib-26399', 'matplotlib__matplotlib-26466', 'matplotlib__matplotlib-26469', 'matplotlib__matplotlib-26472', 'matplotlib__matplotlib-26479', 'matplotlib__matplotlib-26532', 'pydata__xarray-2905', 'pydata__xarray-2922', 'pydata__xarray-3095', 'pydata__xarray-3114', 'pydata__xarray-3151', 'pydata__xarray-3156', 'pydata__xarray-3159', 'pydata__xarray-3239', 'pydata__xarray-3302', 'pydata__xarray-3305', 'pydata__xarray-3338', 'pydata__xarray-3364', 'pydata__xarray-3406', 'pydata__xarray-3520', 'pydata__xarray-3527', 'pydata__xarray-3631', 'pydata__xarray-3635', 'pydata__xarray-3637', 'pydata__xarray-3649', 'pydata__xarray-3677', 'pydata__xarray-3733', 'pydata__xarray-3812', 'pydata__xarray-3905', 'pydata__xarray-3976', 'pydata__xarray-3979', 'pydata__xarray-3993', 'pydata__xarray-4075', 'pydata__xarray-4094', 'pydata__xarray-4098', 'pydata__xarray-4182', 'pydata__xarray-4184', 'pydata__xarray-4248', 'pydata__xarray-4339', 'pydata__xarray-4356', 'pydata__xarray-4419', 'pydata__xarray-4423', 'pydata__xarray-4442', 'pydata__xarray-4493', 'pydata__xarray-4510', 'pydata__xarray-4629', 'pydata__xarray-4683', 'pydata__xarray-4684', 'pydata__xarray-4687', 'pydata__xarray-4695', 'pydata__xarray-4750', 'pydata__xarray-4758', 'pydata__xarray-4759', 'pydata__xarray-4767', 'pydata__xarray-4802', 'pydata__xarray-4819', 'pydata__xarray-4827', 'pydata__xarray-4879', 'pydata__xarray-4911', 'pydata__xarray-4939', 'pydata__xarray-4940', 'pydata__xarray-4966', 'pydata__xarray-4994', 'pydata__xarray-5033', 'pydata__xarray-5126', 'pydata__xarray-5131', 'pydata__xarray-5180', 'pydata__xarray-5187', 'pydata__xarray-5233', 'pydata__xarray-5362', 'pydata__xarray-5365', 'pydata__xarray-5455', 'pydata__xarray-5580', 'pydata__xarray-5662', 'pydata__xarray-5682', 'pydata__xarray-5731', 'pydata__xarray-6135', 'pydata__xarray-6386', 'pydata__xarray-6394', 'pydata__xarray-6400', 'pydata__xarray-6461', 'pydata__xarray-6548', 'pydata__xarray-6598', 'pydata__xarray-6599', 'pydata__xarray-6601', 'pydata__xarray-6721', 'pydata__xarray-6744', 'pydata__xarray-6798', 'pydata__xarray-6804', 'pydata__xarray-6823', 'pydata__xarray-6857', 'pydata__xarray-6882', 'pydata__xarray-6889', 'pydata__xarray-6938', 'pydata__xarray-6971', 'pydata__xarray-6992', 'pydata__xarray-6999', 'pydata__xarray-7003', 'pydata__xarray-7019', 'pydata__xarray-7052', 'pydata__xarray-7089', 'pydata__xarray-7101', 'pydata__xarray-7105', 'pydata__xarray-7112', 'pydata__xarray-7120', 'pydata__xarray-7147', 'pydata__xarray-7150', 'pydata__xarray-7179', 'pydata__xarray-7203', 'pydata__xarray-7229', 'pydata__xarray-7233', 'pydata__xarray-7347', 'pydata__xarray-7391', 'pydata__xarray-7393', 'pydata__xarray-7400', 'pydata__xarray-7444', 'pytest-dev__pytest-10482', 'scikit-learn__scikit-learn-10198', 'scikit-learn__scikit-learn-10297', 'scikit-learn__scikit-learn-10306', 'scikit-learn__scikit-learn-10331', 'scikit-learn__scikit-learn-10377', 'scikit-learn__scikit-learn-10382', 'scikit-learn__scikit-learn-10397', 'scikit-learn__scikit-learn-10427', 'scikit-learn__scikit-learn-10428', 'scikit-learn__scikit-learn-10443', 'scikit-learn__scikit-learn-10452', 'scikit-learn__scikit-learn-10459', 'scikit-learn__scikit-learn-10471', 'scikit-learn__scikit-learn-10483', 'scikit-learn__scikit-learn-10495', 'scikit-learn__scikit-learn-10508', 'scikit-learn__scikit-learn-10558', 'scikit-learn__scikit-learn-10577', 'scikit-learn__scikit-learn-10581', 'scikit-learn__scikit-learn-10687', 'scikit-learn__scikit-learn-10774', 'scikit-learn__scikit-learn-10777', 'scikit-learn__scikit-learn-10803', 'scikit-learn__scikit-learn-10844', 'scikit-learn__scikit-learn-10870', 'scikit-learn__scikit-learn-10881', 'scikit-learn__scikit-learn-10899', 'scikit-learn__scikit-learn-10908', 'scikit-learn__scikit-learn-10913', 'scikit-learn__scikit-learn-10949', 'scikit-learn__scikit-learn-10982', 'scikit-learn__scikit-learn-10986', 'scikit-learn__scikit-learn-11040', 'scikit-learn__scikit-learn-11042', 'scikit-learn__scikit-learn-11043', 'scikit-learn__scikit-learn-11151', 'scikit-learn__scikit-learn-11160', 'scikit-learn__scikit-learn-11206', 'scikit-learn__scikit-learn-11235', 'scikit-learn__scikit-learn-11243', 'scikit-learn__scikit-learn-11264', 'scikit-learn__scikit-learn-11281', 'scikit-learn__scikit-learn-11310', 'scikit-learn__scikit-learn-11315', 'scikit-learn__scikit-learn-11333', 'scikit-learn__scikit-learn-11346', 'scikit-learn__scikit-learn-11391', 'scikit-learn__scikit-learn-11496', 'scikit-learn__scikit-learn-11542', 'scikit-learn__scikit-learn-11574', 'scikit-learn__scikit-learn-11578', 'scikit-learn__scikit-learn-11585', 'scikit-learn__scikit-learn-11596', 'scikit-learn__scikit-learn-11635', 'scikit-learn__scikit-learn-12258', 'scikit-learn__scikit-learn-12421', 'scikit-learn__scikit-learn-12443', 'scikit-learn__scikit-learn-12462', 'scikit-learn__scikit-learn-12471', 'scikit-learn__scikit-learn-12486', 'scikit-learn__scikit-learn-12557', 'scikit-learn__scikit-learn-12583', 'scikit-learn__scikit-learn-12585', 'scikit-learn__scikit-learn-12625', 'scikit-learn__scikit-learn-12626', 'scikit-learn__scikit-learn-12656', 'scikit-learn__scikit-learn-12682', 'scikit-learn__scikit-learn-12704', 'scikit-learn__scikit-learn-12733', 'scikit-learn__scikit-learn-12758', 'scikit-learn__scikit-learn-12760', 'scikit-learn__scikit-learn-12784', 'scikit-learn__scikit-learn-12827', 'scikit-learn__scikit-learn-12834', 'scikit-learn__scikit-learn-12860', 'scikit-learn__scikit-learn-12908', 'scikit-learn__scikit-learn-12938', 'scikit-learn__scikit-learn-12961', 'scikit-learn__scikit-learn-12973', 'scikit-learn__scikit-learn-12983', 'scikit-learn__scikit-learn-12989', 'scikit-learn__scikit-learn-13010', 'scikit-learn__scikit-learn-13013', 'scikit-learn__scikit-learn-13017', 'scikit-learn__scikit-learn-13046', 'scikit-learn__scikit-learn-13087', 'scikit-learn__scikit-learn-13124', 'scikit-learn__scikit-learn-13135', 'scikit-learn__scikit-learn-13142', 'scikit-learn__scikit-learn-13143', 'scikit-learn__scikit-learn-13157', 'scikit-learn__scikit-learn-13165', 'scikit-learn__scikit-learn-13174', 'scikit-learn__scikit-learn-13221', 'scikit-learn__scikit-learn-13241', 'scikit-learn__scikit-learn-13253', 'scikit-learn__scikit-learn-13280', 'scikit-learn__scikit-learn-13283', 'scikit-learn__scikit-learn-13302', 'scikit-learn__scikit-learn-13313', 'scikit-learn__scikit-learn-13328', 'scikit-learn__scikit-learn-13333', 'scikit-learn__scikit-learn-13363', 'scikit-learn__scikit-learn-13368', 'scikit-learn__scikit-learn-13392', 'scikit-learn__scikit-learn-13436', 'scikit-learn__scikit-learn-13439', 'scikit-learn__scikit-learn-13447', 'scikit-learn__scikit-learn-13454', 'scikit-learn__scikit-learn-13467', 'scikit-learn__scikit-learn-13472', 'scikit-learn__scikit-learn-13485', 'scikit-learn__scikit-learn-13496', 'scikit-learn__scikit-learn-13497', 'scikit-learn__scikit-learn-13536', 'scikit-learn__scikit-learn-13549', 'scikit-learn__scikit-learn-13554', 'scikit-learn__scikit-learn-13584', 'scikit-learn__scikit-learn-13618', 'scikit-learn__scikit-learn-13620', 'scikit-learn__scikit-learn-13628', 'scikit-learn__scikit-learn-13641', 'scikit-learn__scikit-learn-13704', 'scikit-learn__scikit-learn-13726', 'scikit-learn__scikit-learn-13779', 'scikit-learn__scikit-learn-13780', 'scikit-learn__scikit-learn-13828', 'scikit-learn__scikit-learn-13864', 'scikit-learn__scikit-learn-13877', 'scikit-learn__scikit-learn-13910', 'scikit-learn__scikit-learn-13915', 'scikit-learn__scikit-learn-13933', 'scikit-learn__scikit-learn-13960', 'scikit-learn__scikit-learn-13974', 'scikit-learn__scikit-learn-13983', 'scikit-learn__scikit-learn-14012', 'scikit-learn__scikit-learn-14024', 'scikit-learn__scikit-learn-14053', 'scikit-learn__scikit-learn-14067', 'scikit-learn__scikit-learn-14087', 'scikit-learn__scikit-learn-14092', 'scikit-learn__scikit-learn-14114', 'scikit-learn__scikit-learn-14125', 'scikit-learn__scikit-learn-14141', 'scikit-learn__scikit-learn-14237', 'scikit-learn__scikit-learn-14309', 'scikit-learn__scikit-learn-14430', 'scikit-learn__scikit-learn-14450', 'scikit-learn__scikit-learn-14458', 'scikit-learn__scikit-learn-14464', 'scikit-learn__scikit-learn-14496', 'scikit-learn__scikit-learn-14520', 'scikit-learn__scikit-learn-14544', 'scikit-learn__scikit-learn-14591', 'scikit-learn__scikit-learn-14629', 'scikit-learn__scikit-learn-14704', 'scikit-learn__scikit-learn-14706', 'scikit-learn__scikit-learn-14710', 'scikit-learn__scikit-learn-14732', 'scikit-learn__scikit-learn-14764', 'scikit-learn__scikit-learn-14806', 'scikit-learn__scikit-learn-14869', 'scikit-learn__scikit-learn-14878', 'scikit-learn__scikit-learn-14890', 'scikit-learn__scikit-learn-14894', 'scikit-learn__scikit-learn-14898', 'scikit-learn__scikit-learn-14908', 'scikit-learn__scikit-learn-14983', 'scikit-learn__scikit-learn-14999', 'scikit-learn__scikit-learn-15028', 'scikit-learn__scikit-learn-15084', 'scikit-learn__scikit-learn-15086', 'scikit-learn__scikit-learn-15094', 'scikit-learn__scikit-learn-15096', 'scikit-learn__scikit-learn-15100', 'scikit-learn__scikit-learn-15119', 'scikit-learn__scikit-learn-15120', 'scikit-learn__scikit-learn-15138', 'scikit-learn__scikit-learn-15393', 'scikit-learn__scikit-learn-15495', 'scikit-learn__scikit-learn-15512', 'scikit-learn__scikit-learn-15524', 'scikit-learn__scikit-learn-15535', 'scikit-learn__scikit-learn-15625', 'scikit-learn__scikit-learn-3840', 'scikit-learn__scikit-learn-7760', 'scikit-learn__scikit-learn-8554', 'scikit-learn__scikit-learn-9274', 'scikit-learn__scikit-learn-9288', 'scikit-learn__scikit-learn-9304', 'scikit-learn__scikit-learn-9775', 'scikit-learn__scikit-learn-9939', 'sphinx-doc__sphinx-11311', 'sphinx-doc__sphinx-7910', 'sympy__sympy-12812', 'sympy__sympy-14248', 'sympy__sympy-15222', 'sympy__sympy-19201'}
```

#####
            FASTLANE_RSPEC_JQ_TRANSFORM

  `module-attribute`

```
FASTLANE_RSPEC_JQ_TRANSFORM = 'tail -n +2 | jq -r \'.examples[] | "\\(.description) - \\(.id) - \\(.status)"\''
```

#####
            FPM_RSPEC_JQ_TRANSFORM

  `module-attribute`

```
FPM_RSPEC_JQ_TRANSFORM = 'sed -n \'/^{/,$p\' | jq -r \'.examples[] | "\\(.description) - \\(.status)"\''
```

#####
            RUBOCOP_RSPEC_JQ_TRANSFORM

  `module-attribute`

```
RUBOCOP_RSPEC_JQ_TRANSFORM = strip()
```

#####
            SPECS_JEKYLL

  `module-attribute`

```
SPECS_JEKYLL = {'9141': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['script/bootstrap'], 'test_cmd': ['bundle exec ruby -I test test/test_site.rb -v -n "/static files/"']}, '8761': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['script/bootstrap'], 'test_cmd': ['bundle exec cucumber --publish-quiet --format progress --no-color features/post_data.feature:6 features/post_data.feature:30']}, '8047': {'docker_specs': {'ruby_version': '3.3'}, 'pre_install': ["sed -i '/^[[:space:]]*install_if.*mingw/,/^[[:space:]]*end/d' Gemfile"], 'install': ['script/bootstrap', 'bundle add webrick'], 'test_cmd': ['bundle exec ruby -I test test/test_filters.rb -v -n "/where_exp filter/"']}, '8167': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['script/bootstrap', 'bundle add webrick'], 'test_cmd': ['bundle exec ruby -I test test/test_utils.rb -v -n "/Utils.slugify/"']}, '8771': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['script/bootstrap'], 'test_cmd': ['bundle exec cucumber --publish-quiet --format progress --no-color features/incremental_rebuild.feature:27 features/incremental_rebuild.feature:70']}}
```

#####
            SPECS_FLUENTD

  `module-attribute`

```
SPECS_FLUENTD = {'4598': {'docker_specs': {'ruby_version': '3.3'}, 'pre_install': ['echo "gem \'console\', \'1.29\'" >> Gemfile'], 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin_helper/test_http_server_helper.rb -v -n '/mount/'"]}, '4311': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/config/test_system_config.rb -v -n '/rotate_age/'"]}, '4655': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_in_http.rb -v -n '/test_add/'"]}, '4030': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ['bundle exec ruby test/plugin/out_forward/test_ack_handler.rb -v']}, '3917': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ['bundle exec ruby test/test_config.rb -v']}, '3640': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin_helper/test_retry_state.rb -v -n '/exponential backoff/'"]}, '3641': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ['bundle exec ruby test/test_supervisor.rb -v']}, '3616': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_in_http.rb -v -n '/test_application/'"]}, '3631': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/test_event_router.rb -v -n '/handle_emits_error/'"]}, '3466': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_in_tail.rb -v -n '/test_should_replace_target_info/'"]}, '3328': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_in_tail.rb -v -n '/test_ENOENT_error_after_setup_watcher/'"]}, '3608': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_output_as_buffered_retries.rb -v -n '/retry_max_times/'"]}}
```

#####
            SPECS_FASTLANE

  `module-attribute`

```
SPECS_FASTLANE = {'21857': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/lane_manager_base_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '20958': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/actions_specs/import_from_git_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '20642': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./frameit/spec/device_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '19765': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/actions_specs/download_dsyms_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '20975': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./match/spec/storage/s3_storage_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '19304': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/actions_specs/zip_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '19207': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/actions_specs/zip_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}}
```

#####
            SPECS_FPM

  `module-attribute`

```
SPECS_FPM = {'1850': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/fpm/package/empty_spec.rb --no-color --format json | {FPM_RSPEC_JQ_TRANSFORM}']}, '1829': {'docker_specs': {'ruby_version': '3.1'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/fpm/package/deb_spec.rb --no-color --format json | {FPM_RSPEC_JQ_TRANSFORM}']}}
```

#####
            SPECS_FAKER

  `module-attribute`

```
SPECS_FAKER = {'2970': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/faker/default/test_faker_internet.rb -v -n '/email/'"]}, '2705': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/faker/default/test_faker_internet.rb -v -n '/password/'"]}}
```

#####
            SPECS_RUBOCOP

  `module-attribute`

```
SPECS_RUBOCOP = {'13705': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/lint/out_of_range_regexp_ref_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13687': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/lint/safe_navigation_chain_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13680': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/redundant_line_continuation_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13668': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/sole_nested_conditional_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13627': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/multiple_comparison_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13653': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/access_modifier_declarations_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13579': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/layout/line_continuation_spacing_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13560': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/file_null_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13503': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/dig_chain_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13479': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/layout/leading_comment_space_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13431': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/layout/empty_lines_around_method_body_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13424': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/safe_navigation_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13393': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/guard_clause_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13396': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/redundant_parentheses_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13375': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cli_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13362': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/redundant_freeze_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}}
```

#####
            MAP_REPO_VERSION_TO_SPECS_RUBY

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_RUBY = {'jekyll/jekyll': SPECS_JEKYLL, 'fluent/fluentd': SPECS_FLUENTD, 'fastlane/fastlane': SPECS_FASTLANE, 'jordansissel/fpm': SPECS_FPM, 'faker-ruby/faker': SPECS_FAKER, 'rubocop/rubocop': SPECS_RUBOCOP}
```

#####
            MAP_REPO_TO_INSTALL_RUBY

  `module-attribute`

```
MAP_REPO_TO_INSTALL_RUBY = {}
```

#####
            SPECS_RIPGREP

  `module-attribute`

```
SPECS_RIPGREP = {'2576': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ripgrep --test integration --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package ripgrep --test integration -- regression']}, '2209': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ripgrep --test integration --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package ripgrep --test integration -- regression::r2208 --exact']}}
```

#####
            SPECS_BAT

  `module-attribute`

```
SPECS_BAT = {'3108': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests pag --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests pag']}, '2835': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests header --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests header']}, '2650': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests map_syntax --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests map_syntax']}, '2393': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests cache_ --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests cache_']}, '2201': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests pag --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests pag']}, '2260': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests syntax --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests syntax']}, '1892': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests ignored_suffix_arg --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests ignored_suffix_arg']}, '562': {'docker_specs': {'rust_version': '1.81'}, 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests cache']}}
```

#####
            SPECS_RUFF

  `module-attribute`

```
SPECS_RUFF = {'15626': {'docker_specs': {'rust_version': '1.84'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::flake8_simplify::tests --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::flake8_simplify::tests']}, '15543': {'docker_specs': {'rust_version': '1.84'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::pyupgrade --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::pyupgrade']}, '15443': {'docker_specs': {'rust_version': '1.84'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::flake8_bandit --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::flake8_bandit']}, '15394': {'docker_specs': {'rust_version': '1.83'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::flake8_pie --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::flake8_pie']}, '15356': {'docker_specs': {'rust_version': '1.83'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::pycodestyle --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::pycodestyle']}, '15330': {'docker_specs': {'rust_version': '1.83'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::eradicate --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::eradicate']}, '15309': {'docker_specs': {'rust_version': '1.83'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --no-run'], 'test_cmd': ["cargo test --package ruff_linter 'f52'"]}}
```

#####
            TOKIO_SPECS

  `module-attribute`

```
TOKIO_SPECS = {'6724': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6724.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test io_write_all_buf --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test io_write_all_buf --no-fail-fast']}, '6838': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6838.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test uds_stream --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test uds_stream --no-fail-fast']}, '6752': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6752.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test time_delay_queue --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test time_delay_queue --no-fail-fast']}, '4867': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-4867.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test sync_broadcast --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test sync_broadcast --no-fail-fast']}, '4898': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-4898.Cargo.lock'), 'install': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --locked --features full --test rt_metrics --no-run'], 'test_cmd': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --features full --test rt_metrics']}, '6603': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6603.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test sync_mpsc --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test sync_mpsc --no-fail-fast']}, '6551': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6551.Cargo.lock'), 'install': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --locked --features full --test rt_metrics --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --features full --test rt_metrics --no-fail-fast']}, '4384': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-4384.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package tokio --test net_lookup_host --features full --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package tokio --test net_types_unwind --features full --no-fail-fast']}, '7139': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-7139.Cargo.lock'), 'install': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --locked --test fs_file --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --test fs_file --no-fail-fast']}}
```

#####
            COREUTILS_SPECS

  `module-attribute`

```
COREUTILS_SPECS = {'6690': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test --no-run -- test_cp_cp test_cp_same_file test_cp_multiple_files test_cp_single_file test_cp_no_file'], 'test_cmd': ['cargo test --no-fail-fast -- test_cp_cp test_cp_same_file test_cp_multiple_files test_cp_single_file test_cp_no_file']}, '6731': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test backslash --no-run'], 'test_cmd': ['cargo test backslash --no-fail-fast']}, '6575': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test cksum --no-run'], 'test_cmd': ['cargo test cksum --no-fail-fast']}, '6682': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test mkdir --no-run'], 'test_cmd': ['cargo test mkdir --no-fail-fast']}, '6377': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test test_env --no-run'], 'test_cmd': ['cargo test test_env --no-fail-fast']}}
```

#####
            NUSHELL_SPECS

  `module-attribute`

```
NUSHELL_SPECS = {'13246': {'docker_specs': {'rust_version': '1.77'}, 'install': ['cargo test -p nu-command --no-run --test main find::'], 'build': ['cargo build'], 'test_cmd': ['cargo test -p nu-command --no-fail-fast --test main find::']}, '12950': {'docker_specs': {'rust_version': '1.77'}, 'install': ['cargo test external_arguments --no-run'], 'test_cmd': ['cargo test external_arguments --no-fail-fast']}, '12901': {'docker_specs': {'rust_version': '1.77'}, 'install': ['cargo test --no-run shell::env'], 'test_cmd': ['cargo test --no-fail-fast shell::env']}, '13831': {'docker_specs': {'rust_version': '1.79'}, 'install': ['cargo test -p nu-command --no-run split_column'], 'build': ['cargo build'], 'test_cmd': ['cargo test -p nu-command --no-fail-fast split_column']}, '13605': {'docker_specs': {'rust_version': '1.78'}, 'install': ['cargo test -p nu-command --no-run ls::'], 'build': ['cargo build'], 'test_cmd': ['cargo test -p nu-command --no-fail-fast ls::']}}
```

#####
            AXUM_SPECS

  `module-attribute`

```
AXUM_SPECS = {'2096': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-2096.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::fallback']}, '1934': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-1934.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::fallback']}, '1730': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-1730.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::mod state']}, '1119': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-1119.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib slash --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib slash']}, '734': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-734.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::head']}, '691': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-691.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::nest::nesting_router_at_root --exact']}, '682': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-682.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib trailing --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib trailing -- with_trailing_slash_post without_trailing_slash_post']}}
```

#####
            MAP_REPO_VERSION_TO_SPECS_RUST

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_RUST = {'burntsushi/ripgrep': SPECS_RIPGREP, 'sharkdp/bat': SPECS_BAT, 'astral-sh/ruff': SPECS_RUFF, 'tokio-rs/tokio': TOKIO_SPECS, 'uutils/coreutils': COREUTILS_SPECS, 'nushell/nushell': NUSHELL_SPECS, 'tokio-rs/axum': AXUM_SPECS}
```

#####
            MAP_REPO_TO_INSTALL_RUST

  `module-attribute`

```
MAP_REPO_TO_INSTALL_RUST = {}
```

#####
            BASE_IMAGE_BUILD_DIR

  `module-attribute`

```
BASE_IMAGE_BUILD_DIR = Path('logs/build_images/base')
```

#####
            ENV_IMAGE_BUILD_DIR

  `module-attribute`

```
ENV_IMAGE_BUILD_DIR = Path('logs/build_images/env')
```

#####
            INSTANCE_IMAGE_BUILD_DIR

  `module-attribute`

```
INSTANCE_IMAGE_BUILD_DIR = Path('logs/build_images/instances')
```

#####
            RUN_EVALUATION_LOG_DIR

  `module-attribute`

```
RUN_EVALUATION_LOG_DIR = Path('logs/run_evaluation')
```

#####
            RUN_VALIDATION_LOG_DIR

  `module-attribute`

```
RUN_VALIDATION_LOG_DIR = Path('logs/run_validation')
```

#####
            FAIL_TO_PASS

  `module-attribute`

```
FAIL_TO_PASS = 'FAIL_TO_PASS'
```

#####
            FAIL_TO_FAIL

  `module-attribute`

```
FAIL_TO_FAIL = 'FAIL_TO_FAIL'
```

#####
            PASS_TO_PASS

  `module-attribute`

```
PASS_TO_PASS = 'PASS_TO_PASS'
```

#####
            PASS_TO_FAIL

  `module-attribute`

```
PASS_TO_FAIL = 'PASS_TO_FAIL'
```

#####
            KEY_INSTANCE_ID

  `module-attribute`

```
KEY_INSTANCE_ID = 'instance_id'
```

#####
            KEY_MODEL

  `module-attribute`

```
KEY_MODEL = 'model_name_or_path'
```

#####
            KEY_PREDICTION

  `module-attribute`

```
KEY_PREDICTION = 'model_patch'
```

#####
            DOCKER_PATCH

  `module-attribute`

```
DOCKER_PATCH = '/tmp/patch.diff'
```

#####
            DOCKER_USER

  `module-attribute`

```
DOCKER_USER = 'root'
```

#####
            DOCKER_WORKDIR

  `module-attribute`

```
DOCKER_WORKDIR = '/testbed'
```

#####
            LOG_REPORT

  `module-attribute`

```
LOG_REPORT = 'report.json'
```

#####
            LOG_INSTANCE

  `module-attribute`

```
LOG_INSTANCE = 'run_instance.log'
```

#####
            LOG_TEST_OUTPUT

  `module-attribute`

```
LOG_TEST_OUTPUT = 'test_output.txt'
```

#####
            UTF8

  `module-attribute`

```
UTF8 = 'utf-8'
```

#####
            APPLY_PATCH_FAIL

  `module-attribute`

```
APPLY_PATCH_FAIL = '>>>>> Patch Apply Failed'
```

#####
            APPLY_PATCH_PASS

  `module-attribute`

```
APPLY_PATCH_PASS = '>>>>> Applied Patch'
```

#####
            INSTALL_FAIL

  `module-attribute`

```
INSTALL_FAIL = '>>>>> Init Failed'
```

#####
            INSTALL_PASS

  `module-attribute`

```
INSTALL_PASS = '>>>>> Init Succeeded'
```

#####
            INSTALL_TIMEOUT

  `module-attribute`

```
INSTALL_TIMEOUT = '>>>>> Init Timed Out'
```

#####
            RESET_FAILED

  `module-attribute`

```
RESET_FAILED = '>>>>> Reset Failed'
```

#####
            TESTS_ERROR

  `module-attribute`

```
TESTS_ERROR = '>>>>> Tests Errored'
```

#####
            TESTS_FAILED

  `module-attribute`

```
TESTS_FAILED = '>>>>> Some Tests Failed'
```

#####
            TESTS_PASSED

  `module-attribute`

```
TESTS_PASSED = '>>>>> All Tests Passed'
```

#####
            TESTS_TIMEOUT

  `module-attribute`

```
TESTS_TIMEOUT = '>>>>> Tests Timed Out'
```

#####
            START_TEST_OUTPUT

  `module-attribute`

```
START_TEST_OUTPUT = '>>>>> Start Test Output'
```

#####
            END_TEST_OUTPUT

  `module-attribute`

```
END_TEST_OUTPUT = '>>>>> End Test Output'
```

#####
            NON_TEST_EXTS

  `module-attribute`

```
NON_TEST_EXTS = ['.json', '.png', 'csv', '.txt', '.md', '.jpg', '.jpeg', '.pkl', '.yml', '.yaml', '.toml']
```

#####
            SWE_BENCH_URL_RAW

  `module-attribute`

```
SWE_BENCH_URL_RAW = 'https://raw.githubusercontent.com/'
```

#####
            DEFAULT_DOCKER_SPECS

  `module-attribute`

```
DEFAULT_DOCKER_SPECS = {'conda_version': 'py311_23.11.0-2', 'node_version': '21.6.2', 'pnpm_version': '9.5.0', 'python_version': '3.9', 'ubuntu_version': '22.04'}
```

#####
            FAIL_ONLY_REPOS

  `module-attribute`

```
FAIL_ONLY_REPOS = {'chartjs/Chart.js', 'processing/p5.js', 'markedjs/marked'}
```

#####
            MAP_REPO_VERSION_TO_SPECS

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS = {None: MAP_REPO_VERSION_TO_SPECS_C, None: MAP_REPO_VERSION_TO_SPECS_GO, None: MAP_REPO_VERSION_TO_SPECS_JAVA, None: MAP_REPO_VERSION_TO_SPECS_JS, None: MAP_REPO_VERSION_TO_SPECS_PHP, None: MAP_REPO_VERSION_TO_SPECS_PY, None: MAP_REPO_VERSION_TO_SPECS_RUBY, None: MAP_REPO_VERSION_TO_SPECS_RUST}
```

#####
            MAP_REPO_TO_INSTALL

  `module-attribute`

```
MAP_REPO_TO_INSTALL = {None: MAP_REPO_TO_INSTALL_C, None: MAP_REPO_TO_INSTALL_GO, None: MAP_REPO_TO_INSTALL_JAVA, None: MAP_REPO_TO_INSTALL_JS, None: MAP_REPO_TO_INSTALL_PHP, None: MAP_REPO_TO_INSTALL_PY, None: MAP_REPO_TO_INSTALL_RUBY, None: MAP_REPO_TO_INSTALL_RUST}
```

#####
            MAP_REPO_TO_EXT

  `module-attribute`

```
MAP_REPO_TO_EXT = {None: {k: 'c' for k in (keys())}, None: {k: 'go' for k in (keys())}, None: {k: 'java' for k in (keys())}, None: {k: 'js' for k in (keys())}, None: {k: 'php' for k in (keys())}, None: {k: 'py' for k in (keys())}, None: {k: 'rb' for k in (keys())}, None: {k: 'rs' for k in (keys())}}
```

#####
            LATEST

  `module-attribute`

```
LATEST = 'latest'
```

#####
            USE_X86

  `module-attribute`

```
USE_X86 = USE_X86_PY
```

#####
            REPO_BASE_COMMIT_BRANCH

  `module-attribute`

```
REPO_BASE_COMMIT_BRANCH = {'sympy/sympy': {'cffd4e0f86fefd4802349a9f9b19ed70934ea354': '1.7', '70381f282f2d9d039da860e391fe51649df2779d': 'sympy-1.5.1'}, 'pytest-dev/pytest': {'8aba863a634f40560e25055d179220f0eefabe9a': '4.6.x'}}
```

#####
            SWEbenchInstance

              Bases: `TypedDict`

######
            repo

  `instance-attribute`

```
repo: str
```

######
            instance_id

  `instance-attribute`

```
instance_id: str
```

######
            base_commit

  `instance-attribute`

```
base_commit: str
```

######
            patch

  `instance-attribute`

```
patch: str
```

######
            test_patch

  `instance-attribute`

```
test_patch: str
```

######
            problem_statement

  `instance-attribute`

```
problem_statement: str
```

######
            hints_text

  `instance-attribute`

```
hints_text: str
```

######
            created_at

  `instance-attribute`

```
created_at: str
```

######
            version

  `instance-attribute`

```
version: str
```

######
            FAIL_TO_PASS

  `instance-attribute`

```
FAIL_TO_PASS: str
```

######
            PASS_TO_PASS

  `instance-attribute`

```
PASS_TO_PASS: str
```

######
            environment_setup_commit

  `instance-attribute`

```
environment_setup_commit: str
```

#####
            ResolvedStatus

              Bases: `Enum`

######
            NO

  `class-attribute` `instance-attribute`

```
NO = 'RESOLVED_NO'
```

######
            PARTIAL

  `class-attribute` `instance-attribute`

```
PARTIAL = 'RESOLVED_PARTIAL'
```

######
            FULL

  `class-attribute` `instance-attribute`

```
FULL = 'RESOLVED_FULL'
```

#####
            TestStatus

              Bases: `Enum`

######
            FAILED

  `class-attribute` `instance-attribute`

```
FAILED = 'FAILED'
```

######
            PASSED

  `class-attribute` `instance-attribute`

```
PASSED = 'PASSED'
```

######
            SKIPPED

  `class-attribute` `instance-attribute`

```
SKIPPED = 'SKIPPED'
```

######
            ERROR

  `class-attribute` `instance-attribute`

```
ERROR = 'ERROR'
```

######
            XFAIL

  `class-attribute` `instance-attribute`

```
XFAIL = 'XFAIL'
```

#####
            EvalType

              Bases: `Enum`

######
            PASS_AND_FAIL

  `class-attribute` `instance-attribute`

```
PASS_AND_FAIL = 'pass_and_fail'
```

######
            FAIL_ONLY

  `class-attribute` `instance-attribute`

```
FAIL_ONLY = 'fail_only'
```

#####
            PatchType

              Bases: `Enum`

######
            PATCH_GOLD

  `class-attribute` `instance-attribute`

```
PATCH_GOLD = 'gold'
```

######
            PATCH_PRED

  `class-attribute` `instance-attribute`

```
PATCH_PRED = 'pred'
```

######
            PATCH_PRED_TRY

  `class-attribute` `instance-attribute`

```
PATCH_PRED_TRY = 'pred_try'
```

######
            PATCH_PRED_MINIMAL

  `class-attribute` `instance-attribute`

```
PATCH_PRED_MINIMAL = 'pred_minimal'
```

######
            PATCH_PRED_MINIMAL_TRY

  `class-attribute` `instance-attribute`

```
PATCH_PRED_MINIMAL_TRY = 'pred_minimal_try'
```

######
            PATCH_TEST

  `class-attribute` `instance-attribute`

```
PATCH_TEST = 'test'
```

######
            __str__

```
__str__()
```

Source code in `swebench/harness/constants/__init__.py` | 103
104 | def __str__(self):
    return self.value |
| ------- | ---------------------------------------- |

#####
            make_lombok_pre_install_script

```
make_lombok_pre_install_script(tests: List[str]) -> List[str]
```

There's no way to run individual tests out of the box, so this script
modifies the xml file that defines test scripts to run individual tests with
`ant test.instance`.

Source code in `swebench/harness/constants/java.py` | 5
 6
 7
 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31 | def make_lombok_pre_install_script(tests: List[str]) -> List[str]:
    """
    There's no way to run individual tests out of the box, so this script
    modifies the xml file that defines test scripts to run individual tests with
    `ant test.instance`.
    """
    tests_xml = "\n".join(rf'<test name="{test}" />' for test in tests)
    xml = rf"""
    <target name="test.instance" depends="test.compile, test.formatter.compile" description="Runs test cases for the swe-bench instance">
      <junit printsummary="yes" fork="true" forkmode="once" haltonfailure="no">
        <formatter classname="lombok.ant.SimpleTestFormatter" usefile="false" unless="tests.quiet" />
        <classpath location="build/ant" />
        <classpath refid="cp.test" />
        <classpath refid="cp.stripe" />
        <classpath refid="packing.basedirs.path" />
        <classpath location="build/tests" />
        <classpath location="build/teststubs" />
        {tests_xml}
      </junit>
    </target>
    """
    build_file = "buildScripts/tests.ant.xml"
    escaped_xml = shlex.quote(xml.strip())

    return [
        f"{{ head -n -1 {build_file}; echo {escaped_xml}; tail -n 1 {build_file}; }} > temp_file && mv temp_file {build_file}"
    ] |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

#####
            make_lucene_pre_install_script

```
make_lucene_pre_install_script() -> List[str]
```

This script modifies the gradle config to print all test results, including
passing tests.

Source code in `swebench/harness/constants/java.py` | 34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
80
81
82
83
84
85
86
87 | def make_lucene_pre_install_script() -> List[str]:
    """
    This script modifies the gradle config to print all test results, including
    passing tests.
    """
    gradle_file = "gradle/testing/defaults-tests.gradle"

    new_content = """testLogging {
  showStandardStreams = true
  // set options for log level LIFECYCLE
  events TestLogEvent.FAILED,
         TestLogEvent.PASSED,
         TestLogEvent.SKIPPED,
         TestLogEvent.STANDARD_OUT
  exceptionFormat TestExceptionFormat.FULL
  showExceptions true
  showCauses true
  showStackTraces true

  // set options for log level DEBUG and INFO
  debug {
      events TestLogEvent.STARTED,
             TestLogEvent.FAILED,
             TestLogEvent.PASSED,
             TestLogEvent.SKIPPED,
             TestLogEvent.STANDARD_ERROR,
             TestLogEvent.STANDARD_OUT
      exceptionFormat TestExceptionFormat.FULL
  }
  info.events = debug.events
  info.exceptionFormat = debug.exceptionFormat

  afterSuite { desc, result ->
      if (!desc.parent) { // will match the outermost suite
          def output = "Results: ${result.resultType} (${result.testCount} tests, ${result.successfulTestCount} passed, ${result.failedTestCount} failed, ${result.skippedTestCount} skipped)"
          def startItem = '\|  ', endItem = '  \|'
          def repeatLength = startItem.length() + output.length() + endItem.length()
          println('\\n' + ('-' * repeatLength) + '\\n' + startItem + output + endItem + '\\n' + ('-' * repeatLength))
      }
  }
}"""

    return [
        f"""
sed -i '
/testLogging {{/,/}}/{{
  /testLogging {{/r /dev/stdin
  d
}}
' {gradle_file} << 'EOF'
{new_content}
EOF
""".strip()
    ] |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            make_rxjava_pre_install_script

```
make_rxjava_pre_install_script() -> List[str]
```

This script modifies the gradle config to print all test results, including
passing tests.

Source code in `swebench/harness/constants/java.py` | 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150 | def make_rxjava_pre_install_script() -> List[str]:
    """
    This script modifies the gradle config to print all test results, including
    passing tests.
    """
    gradle_file = "build.gradle"

    new_content = """testLogging {
    outputs.upToDateWhen { false }
    showStandardStreams = true
    showStackTraces = true

    // Show output for all logging levels
    events = ['passed', 'skipped', 'failed', 'standardOut', 'standardError']

    // set options for log level LIFECYCLE
    events org.gradle.api.tasks.testing.logging.TestLogEvent.FAILED,
           org.gradle.api.tasks.testing.logging.TestLogEvent.PASSED,
           org.gradle.api.tasks.testing.logging.TestLogEvent.SKIPPED,
           org.gradle.api.tasks.testing.logging.TestLogEvent.STANDARD_OUT,
           org.gradle.api.tasks.testing.logging.TestLogEvent.STANDARD_ERROR
    exceptionFormat org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    showExceptions true
    showCauses true
    showStackTraces true

    // set options for log level DEBUG and INFO
    debug {
        events org.gradle.api.tasks.testing.logging.TestLogEvent.STARTED,
               org.gradle.api.tasks.testing.logging.TestLogEvent.FAILED,
               org.gradle.api.tasks.testing.logging.TestLogEvent.PASSED,
               org.gradle.api.tasks.testing.logging.TestLogEvent.SKIPPED,
               org.gradle.api.tasks.testing.logging.TestLogEvent.STANDARD_ERROR,
               org.gradle.api.tasks.testing.logging.TestLogEvent.STANDARD_OUT
        exceptionFormat org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
    info.events = debug.events
    info.exceptionFormat = debug.exceptionFormat

    afterSuite { desc, result ->
        if (!desc.parent) { // will match the outermost suite
            def output = "Results: ${result.resultType} (${result.testCount} tests, ${result.successfulTestCount} passed, ${result.failedTestCount} failed, ${result.skippedTestCount} skipped)"
            def startItem = '\|  ', endItem = '  \|'
            def repeatLength = startItem.length() + output.length() + endItem.length()
            println('\\n' + ('-' * repeatLength) + '\\n' + startItem + output + endItem + '\\n' + ('-' * repeatLength))
        }
    }
}"""

    return [
        f"""
sed -i '
/testLogging {{/,/}}/{{
  /testLogging {{/r /dev/stdin
  d
}}
' {gradle_file} << 'EOF'
{new_content}
EOF
""".strip()
    ] |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            c

######
            SPECS_REDIS

  `module-attribute`

```
SPECS_REDIS = {'13115': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/scripting']}, '12472': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/acl --only "/.*ACL GETUSER.*"']}, '12272': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/string --only "/.*(GETRANGE|SETRANGE).*"']}, '11734': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/bitops']}, '10764': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/zset --only "BZMPOP"']}, '10095': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/list --only "/.*(LPOP|RPOP)"']}, '9733': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/introspection-2']}, '10068': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/stream --only "/*XTRIM*"']}, '11631': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/geo --only "/.*GEOSEARCH .*"']}, '11510': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/introspection --only "/.*MONITOR.*"']}, '11279': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/acl']}, '13338': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/type/stream-cgroups']}}
```

######
            SPECS_JQ

  `module-attribute`

```
SPECS_JQ = {None: {k: {'build': ['git submodule update --init', 'autoreconf -fi', './configure --with-oniguruma=builtin', 'make clean', 'touch src/parser.y src/lexer.l', 'make -j$(nproc)'], 'test_cmd': ['make check']} for k in ['2839', '2650', '2235', '2658', '2750', '2681', '2919', '2598', '2728']}}
```

######
            SPECS_JSON

  `module-attribute`

```
SPECS_JSON = {'4237': {'build': ['mkdir -p build', 'cd build', 'cmake ..', 'make test-udt_cpp11', 'cd ..'], 'test_cmd': ['./build/tests/test-udt_cpp11 -s -r=xml']}}
```

######
            SPECS_MICROPYTHON

  `module-attribute`

```
SPECS_MICROPYTHON = {'15898': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate'], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard', 'gcc -shared -o tests/ports/unix/ffi_lib.so tests/ports/unix/ffi_lib.c'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -i string_format']}, '13569': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate'], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard', 'gcc -shared -o tests/ports/unix/ffi_lib.so tests/ports/unix/ffi_lib.c'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -i try']}, '13039': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate'], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard', 'gcc -shared -o tests/unix/ffi_lib.so tests/unix/ffi_lib.c'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -i slice']}, '12158': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate'], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard', 'gcc -shared -o tests/unix/ffi_lib.so tests/unix/ffi_lib.c'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -d thread']}, '10095': {'pre_install': ['python -m venv .venv', 'source .venv/bin/activate', "sed -i 's/uint mp_import_stat/mp_import_stat_t mp_import_stat/' mpy-cross/main.c"], 'build': ['source ./tools/ci.sh', 'ci_unix_build_helper VARIANT=standard'], 'test_cmd': ['cd tests', 'MICROPY_CPYTHON3=python3 MICROPY_MICROPYTHON=../ports/unix/build-standard/micropython ./run-tests.py -i basics/fun']}}
```

######
            SPECS_VALKEY

  `module-attribute`

```
SPECS_VALKEY = {'928': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/cluster/replica-migration --only "/.*NOREPLICAS.*"']}, '790': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/cluster/cluster-shards']}, '1499': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/introspection-2']}, '1842': {'build': ['make distclean', 'make'], 'test_cmd': ['TERM=dumb ./runtest --durable --single unit/acl --only "/.*ACL LOAD.*"']}}
```

######
            SPECS_FMT

  `module-attribute`

```
SPECS_FMT = {None: {k: {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target ranges-test'], 'test_cmd': ['ctest --test-dir build -V -R ranges-test']} for k in ['3863', '3158', '2457']}, None: {k: {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target format-test'], 'test_cmd': ['ctest --test-dir build -V -R format-test']} for k in ['3901', '3750', '3248', '2317', '2310']}, '3272': {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target xchar-test'], 'test_cmd': ['ctest --test-dir build -V -R xchar-test']}, '3729': {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target std-test'], 'test_cmd': ['ctest --test-dir build -V -R std-test']}, '1683': {'build': ['mkdir -p build', 'cmake -B build -S .', 'cmake --build build --parallel $(nproc) --target printf-test'], 'test_cmd': ['ctest --test-dir build -V -R printf-test']}}
```

######
            MAP_REPO_VERSION_TO_SPECS_C

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_C = {'redis/redis': SPECS_REDIS, 'jqlang/jq': SPECS_JQ, 'nlohmann/json': SPECS_JSON, 'micropython/micropython': SPECS_MICROPYTHON, 'valkey-io/valkey': SPECS_VALKEY, 'fmtlib/fmt': SPECS_FMT}
```

######
            MAP_REPO_TO_INSTALL_C

  `module-attribute`

```
MAP_REPO_TO_INSTALL_C = {}
```

#####
            go

######
            SPECS_CADDY

  `module-attribute`

```
SPECS_CADDY = {'6411': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go mod tidy'], 'test_cmd': ['go test -v . -run "TestReplacerNew*"']}, '6345': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddytest/integration'], 'test_cmd': ['go test -v ./caddytest/integration']}, '6115': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./modules/caddyhttp/reverseproxy'], 'test_cmd': ['go test -v ./modules/caddyhttp/reverseproxy']}, '6051': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddyconfig/caddyfile'], 'test_cmd': ['go test -v ./caddyconfig/caddyfile']}, '5404': {'docker_specs': {'go_version': '1.20.14'}, 'install': ['go test -c ./caddyconfig/caddyfile'], 'test_cmd': ['go test -v ./caddyconfig/caddyfile']}, '6370': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./cmd'], 'test_cmd': ['go test -v ./cmd']}, '6350': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddytest/integration -run "TestCaddyfileAdapt*"'], 'test_cmd': ['go test -v ./caddytest/integration -run "TestCaddyfileAdapt*"']}, '6288': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddytest/integration -run "TestCaddyfileAdapt*"'], 'test_cmd': ['go test -v ./caddytest/integration -run "TestCaddyfileAdapt*"']}, '5995': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddytest/integration -run "^TestUriReplace"'], 'test_cmd': ['go test -v ./caddytest/integration -run "^TestUriReplace"']}, '4943': {'docker_specs': {'go_version': '1.18.10'}, 'install': ['go test -c ./modules/logging'], 'test_cmd': ['go test -v ./modules/logging']}, '5626': {'docker_specs': {'go_version': '1.19.13'}, 'install': ['go test -c ./caddyconfig/httpcaddyfile -run "Test.*Import"'], 'test_cmd': ['go test -v ./caddyconfig/httpcaddyfile -run "Test.*Import"']}, '5761': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./caddyconfig/caddyfile -run "TestLexer.*"'], 'test_cmd': ['go test -v ./caddyconfig/caddyfile -run "TestLexer.*"']}, '5870': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c . -run "TestUnsyncedConfigAccess"'], 'test_cmd': ['go test -v . -run "TestUnsyncedConfigAccess"']}, '4774': {'docker_specs': {'go_version': '1.18.10'}, 'install': ['go test -c ./caddytest/integration -run "TestCaddyfileAdapt*"'], 'test_cmd': ['go test -v ./caddytest/integration -run "TestCaddyfileAdapt*"']}}
```

######
            SPECS_TERRAFORM

  `module-attribute`

```
SPECS_TERRAFORM = {'35611': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./internal/terraform'], 'test_cmd': ['go test -v ./internal/terraform -run "^TestContext2Apply_provisioner"']}, '35543': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./internal/terraform'], 'test_cmd': ['go test -v ./internal/terraform -run "^TestContext2Plan_import"']}, '34900': {'docker_specs': {'go_version': '1.22.12'}, 'install': ['go test -c ./internal/terraform'], 'test_cmd': ['go test -v ./internal/terraform -run "(^TestContext2Apply|^TestContext2Plan).*[Ss]ensitive"']}, '34580': {'docker_specs': {'go_version': '1.21.13'}, 'install': ['go test -c ./internal/command'], 'test_cmd': ['go test -v ./internal/command -run "^TestFmt"']}, '34814': {'docker_specs': {'go_version': '1.22.12'}, 'install': ['go test -c ./internal/builtin/provisioners/remote-exec'], 'test_cmd': ['go test -v ./internal/builtin/provisioners/remote-exec']}}
```

######
            SPECS_PROMETHEUS

  `module-attribute`

```
SPECS_PROMETHEUS = {'14861': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./promql'], 'test_cmd': ['go test -v ./promql -run "^TestEngine"']}, '13845': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./promql ./model/labels'], 'test_cmd': ['go test -v ./promql ./model/labels -run "^(TestRangeQuery|TestLabels)"']}, '12874': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./tsdb'], 'test_cmd': ['go test -v ./tsdb -run "^TestHead"']}, '11859': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./tsdb'], 'test_cmd': ['go test -v ./tsdb -run "^TestSnapshot"']}, '10720': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./promql'], 'test_cmd': ['go test -v ./promql -run "^TestEvaluations"']}, '10633': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./discovery/puppetdb'], 'test_cmd': ['go test -v ./discovery/puppetdb -run "TestPuppetDBRefreshWithParameters"']}, '9248': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./promql'], 'test_cmd': ['go test -v ./promql -run "^TestEvaluations"']}, '15142': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./tsdb'], 'test_cmd': ['go test -v ./tsdb -run "^TestHead"']}}
```

######
            SPECS_HUGO

  `module-attribute`

```
SPECS_HUGO = {'12768': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./markup/goldmark/blockquotes/...'], 'test_cmd': ['go test -v ./markup/goldmark/blockquotes/...']}, '12579': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./resources/page'], 'test_cmd': ['go test -v ./resources/page -run "^TestGroupBy"']}, '12562': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./hugolib/...'], 'test_cmd': ['go test -v ./hugolib/... -run "^TestGetPage[^/]"']}, '12448': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./hugolib/...'], 'test_cmd': ['go test -v ./hugolib/... -run "^TestRebuild"']}, '12343': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./resources/page/...'], 'test_cmd': ['go test -v ./resources/page/... -run "^Test.*Permalink"']}, '12204': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./tpl/tplimpl'], 'test_cmd': ['go test -v ./tpl/tplimpl -run "^TestEmbedded"']}, '12171': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./hugolib'], 'test_cmd': ['go test -v ./hugolib -run "^Test.*Pages"']}}
```

######
            SPECS_GIN

  `module-attribute`

```
SPECS_GIN = {'4003': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test . -v -run "TestMethodNotAllowedNoRoute"']}, '3820': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./binding'], 'test_cmd': ['go test -v ./binding -run "^TestMapping"']}, '3741': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^TestColor"']}, '2755': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^TestTree"']}, '3227': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^TestRedirect"']}, '2121': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c ./...'], 'test_cmd': ['go test -v ./... -run "^Test.*Reader"']}, '1957': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^TestContext.*Bind"']}, '1805': {'docker_specs': {'go_version': '1.23.8'}, 'install': ['go test -c .'], 'test_cmd': ['go test -v . -run "^Test.*Router"']}}
```

######
            MAP_REPO_VERSION_TO_SPECS_GO

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_GO = {'caddyserver/caddy': SPECS_CADDY, 'hashicorp/terraform': SPECS_TERRAFORM, 'prometheus/prometheus': SPECS_PROMETHEUS, 'gohugoio/hugo': SPECS_HUGO, 'gin-gonic/gin': SPECS_GIN}
```

######
            MAP_REPO_TO_INSTALL_GO

  `module-attribute`

```
MAP_REPO_TO_INSTALL_GO = {}
```

#####
            java

######
            SPECS_GSON

  `module-attribute`

```
SPECS_GSON = {'2158': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testByteSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testShortSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testIntSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testLongSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testFloatSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testDoubleSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testPrimitiveIntegerAutoboxedSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testPrimitiveIntegerAutoboxedInASingleElementArraySerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testReallyLongValuesSerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.PrimitiveTest#testPrimitiveLongAutoboxedSerialization']}, '2024': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.functional.FieldNamingTest#testUpperCaseWithUnderscores', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.NamingPolicyTest#testGsonWithUpperCaseUnderscorePolicySerialization', 'mvnd test -B -pl gson -Dtest=com.google.gson.functional.NamingPolicyTest#testGsonWithUpperCaseUnderscorePolicyDeserialiation']}, '2479': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.GsonBuilderTest#testRegisterTypeAdapterForObjectAndJsonElements', 'mvnd test -B -pl gson -Dtest=com.google.gson.GsonBuilderTest#testRegisterTypeHierarchyAdapterJsonElements', 'mvnd test -B -pl gson -Dtest=com.google.gson.GsonBuilderTest#testModificationAfterCreate']}, '2134': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.util.ISO8601UtilsTest#testDateParseInvalidDay', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.util.ISO8601UtilsTest#testDateParseInvalidMonth', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.util.ISO8601UtilsTest#testDateParseWithDefaultTimezone']}, '2061': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonReaderTest#testHasNextEndOfDocument', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testHasNext_endOfDocument', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonReaderTest#testReadEmptyObject', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonReaderTest#testReadEmptyArray', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testSkipValue_emptyJsonObject', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testSkipValue_filledJsonObject']}, '2311': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.JsonPrimitiveTest#testEqualsIntegerAndBigInteger', 'mvnd test -B -pl gson -Dtest=com.google.gson.JsonPrimitiveTest#testLongEqualsBigInteger', 'mvnd test -B -pl gson -Dtest=com.google.gson.JsonPrimitiveTest#testEqualsAcrossTypes']}, '1100': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.DefaultDateTypeAdapterTest#testNullValue', 'mvnd test -B -pl gson -Dtest=com.google.gson.DefaultDateTypeAdapterTest#testDatePattern', 'mvnd test -B -pl gson -Dtest=com.google.gson.DefaultDateTypeAdapterTest#testInvalidDatePattern']}, '1093': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testNonFiniteDoublesWhenLenient', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testNonFiniteBoxedDoublesWhenLenient', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testNonFiniteDoubles', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testNonFiniteBoxedDoubles', 'mvnd test -B -pl gson -Dtest=com.google.gson.stream.JsonWriterTest#testDoubles']}, '1014': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl gson -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testSkipValue_emptyJsonObject', 'mvnd test -B -pl gson -Dtest=com.google.gson.internal.bind.JsonTreeReaderTest#testSkipValue_filledJsonObject']}}
```

######
            SPECS_DRUID

  `module-attribute`

```
SPECS_DRUID = {'15402': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl processing -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl processing -Dtest=org.apache.druid.query.groupby.GroupByQueryQueryToolChestTest#testCacheStrategy', 'mvnd test -B -pl processing -Dtest=org.apache.druid.query.groupby.GroupByQueryQueryToolChestTest#testResultLevelCacheKeyWithSubTotalsSpec', 'mvnd test -B -pl processing -Dtest=org.apache.druid.query.groupby.GroupByQueryQueryToolChestTest#testMultiColumnCacheStrategy']}, '14092': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl processing,cloud/aws-common,cloud/gcp-common -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl server -Dtest=org.apache.druid.discovery.DruidLeaderClientTest#test503ResponseFromServerAndCacheRefresh', 'mvnd test -B -pl server -Dtest=org.apache.druid.discovery.DruidLeaderClientTest#testServerFailureAndRedirect']}, '14136': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl processing -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirstZeroLengthInterval', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirstZeroLengthInterval2', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirstZeroLengthInterval3', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirstZeroLengthInterval4', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapFirstContainsSecond', 'mvnd test -B -pl processing -Dtest=org.apache.druid.timeline.VersionedIntervalTimelineTest#testOverlapSecondContainsFirst']}, '13704': {'docker_specs': {'java_version': '11'}, 'install': ["sed -i 's/<resourceBundle>org.apache.apache.resources:apache-jar-resource-bundle:1.5-SNAPSHOT<\\/resourceBundle>/<resourceBundle>org.apache.apache.resources:apache-jar-resource-bundle:1.5<\\/resourceBundle>/' pom.xml", 'mvn clean install -B -pl processing -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl processing -Dtest=org.apache.druid.query.aggregation.post.ArithmeticPostAggregatorTest#testPow', 'mvnd test -B -pl processing -Dtest=org.apache.druid.query.aggregation.post.ArithmeticPostAggregatorTest#testDiv', 'mvnd test -B -pl processing -Dtest=org.apache.druid.query.aggregation.post.ArithmeticPostAggregatorTest#testQuotient']}, '16875': {'docker_specs': {'java_version': '11'}, 'install': ['mvn clean install -B -pl server -DskipTests -am'], 'test_cmd': ['mvnd test -B -pl server -Dtest=org.apache.druid.server.metrics.WorkerTaskCountStatsMonitorTest#testMonitorWithPeon', 'mvnd test -B -pl server -Dtest=org.apache.druid.server.metrics.WorkerTaskCountStatsMonitorTest#testMonitorWithNulls', 'mvnd test -B -pl server -Dtest=org.apache.druid.server.metrics.WorkerTaskCountStatsMonitorTest#testMonitorIndexer']}}
```

######
            SPECS_JAVAPARSER

  `module-attribute`

```
SPECS_JAVAPARSER = {'4561': {'docker_specs': {'java_version': '17'}, 'build': ['./mvnw clean install -B -pl javaparser-symbol-solver-testing -DskipTests -am'], 'test_cmd': ['./mvnw test -B -pl javaparser-symbol-solver-testing -Dtest=Issue4560Test', './mvnw test -B -pl javaparser-symbol-solver-testing -Dtest=JavaSymbolSolverTest']}, '4538': {'docker_specs': {'java_version': '17'}, 'build': ['./mvnw clean install -B -pl javaparser-core-testing -DskipTests -am'], 'test_cmd': ['./mvnw test -B -pl javaparser-core-testing -Dtest=NodeTest', './mvnw test -B -pl javaparser-core-testing -Dtest=NodePositionTest']}}
```

######
            SPECS_LOMBOK

  `module-attribute`

```
SPECS_LOMBOK = {'3602': {'docker_specs': {'java_version': '11'}, 'pre_install': make_lombok_pre_install_script(['lombok.bytecode.TestPostCompiler']), 'build': ['ant test.compile'], 'test_cmd': ['ant test.instance']}, None: {k: {'docker_specs': {'java_version': '11'}, 'pre_install': make_lombok_pre_install_script(['lombok.transform.TestWithDelombok']), 'build': ['ant test.compile'], 'test_cmd': ['ant test.instance']} for k in ['3312', '3697', '3326', '3674', '3594', '3422', '3215', '3486', '3042', '3052', '2792']}, None: {k: {'docker_specs': {'java_version': '17'}, 'pre_install': make_lombok_pre_install_script(['lombok.transform.TestWithDelombok']), 'build': ['ant test.compile'], 'test_cmd': ['ant test.instance']} for k in ['3571', '3479', '3371', '3350', '3009']}}
```

######
            SPECS_LUCENE

  `module-attribute`

```
SPECS_LUCENE = {'13494': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.facet.TestStringValueFacetCounts']}, '13704': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.search.TestLatLonDocValuesQueries']}, '13301': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests TestXYPoint.testEqualsAndHashCode -Dtests.seed=3ABEFE4D876DD310 -Dtests.nightly=true -Dtests.locale=es-419 -Dtests.timezone=Asia/Ulaanbaatar -Dtests.asserts=true -Dtests.file.encoding=UTF-8']}, '12626': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.index.TestIndexWriter']}, '12212': {'docker_specs': {'java_version': '17'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.facet.TestDrillSideways']}, '13170': {'docker_specs': {'java_version': '21'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.analysis.opennlp.TestOpenNLPSentenceBreakIterator -Ptests.useSecurityManager=false']}, '12196': {'docker_specs': {'java_version': '17'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.queryparser.classic.TestMultiFieldQueryParser']}, '12022': {'docker_specs': {'java_version': '17'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.document.TestLatLonShape']}, '11760': {'docker_specs': {'java_version': '17'}, 'pre_install': make_lucene_pre_install_script(), 'test_cmd': ['./gradlew test --tests org.apache.lucene.queries.intervals.TestIntervalBuilder']}}
```

######
            SPECS_RXJAVA

  `module-attribute`

```
SPECS_RXJAVA = {'7597': {'docker_specs': {'java_version': '11'}, 'pre_install': make_rxjava_pre_install_script(), 'test_cmd': ['./gradlew test --tests io.reactivex.rxjava3.internal.operators.observable.ObservableSwitchTest']}}
```

######
            MAP_REPO_VERSION_TO_SPECS_JAVA

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_JAVA = {'google/gson': SPECS_GSON, 'apache/druid': SPECS_DRUID, 'javaparser/javaparser': SPECS_JAVAPARSER, 'projectlombok/lombok': SPECS_LOMBOK, 'apache/lucene': SPECS_LUCENE, 'reactivex/rxjava': SPECS_RXJAVA}
```

######
            MAP_REPO_TO_INSTALL_JAVA

  `module-attribute`

```
MAP_REPO_TO_INSTALL_JAVA = {}
```

######
            make_lombok_pre_install_script

```
make_lombok_pre_install_script(tests: List[str]) -> List[str]
```

There's no way to run individual tests out of the box, so this script
modifies the xml file that defines test scripts to run individual tests with
`ant test.instance`.

Source code in `swebench/harness/constants/java.py` | 5
 6
 7
 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31 | def make_lombok_pre_install_script(tests: List[str]) -> List[str]:
    """
    There's no way to run individual tests out of the box, so this script
    modifies the xml file that defines test scripts to run individual tests with
    `ant test.instance`.
    """
    tests_xml = "\n".join(rf'<test name="{test}" />' for test in tests)
    xml = rf"""
    <target name="test.instance" depends="test.compile, test.formatter.compile" description="Runs test cases for the swe-bench instance">
      <junit printsummary="yes" fork="true" forkmode="once" haltonfailure="no">
        <formatter classname="lombok.ant.SimpleTestFormatter" usefile="false" unless="tests.quiet" />
        <classpath location="build/ant" />
        <classpath refid="cp.test" />
        <classpath refid="cp.stripe" />
        <classpath refid="packing.basedirs.path" />
        <classpath location="build/tests" />
        <classpath location="build/teststubs" />
        {tests_xml}
      </junit>
    </target>
    """
    build_file = "buildScripts/tests.ant.xml"
    escaped_xml = shlex.quote(xml.strip())

    return [
        f"{{ head -n -1 {build_file}; echo {escaped_xml}; tail -n 1 {build_file}; }} > temp_file && mv temp_file {build_file}"
    ] |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            make_lucene_pre_install_script

```
make_lucene_pre_install_script() -> List[str]
```

This script modifies the gradle config to print all test results, including
passing tests.

Source code in `swebench/harness/constants/java.py` | 34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
80
81
82
83
84
85
86
87 | def make_lucene_pre_install_script() -> List[str]:
    """
    This script modifies the gradle config to print all test results, including
    passing tests.
    """
    gradle_file = "gradle/testing/defaults-tests.gradle"

    new_content = """testLogging {
  showStandardStreams = true
  // set options for log level LIFECYCLE
  events TestLogEvent.FAILED,
         TestLogEvent.PASSED,
         TestLogEvent.SKIPPED,
         TestLogEvent.STANDARD_OUT
  exceptionFormat TestExceptionFormat.FULL
  showExceptions true
  showCauses true
  showStackTraces true

  // set options for log level DEBUG and INFO
  debug {
      events TestLogEvent.STARTED,
             TestLogEvent.FAILED,
             TestLogEvent.PASSED,
             TestLogEvent.SKIPPED,
             TestLogEvent.STANDARD_ERROR,
             TestLogEvent.STANDARD_OUT
      exceptionFormat TestExceptionFormat.FULL
  }
  info.events = debug.events
  info.exceptionFormat = debug.exceptionFormat

  afterSuite { desc, result ->
      if (!desc.parent) { // will match the outermost suite
          def output = "Results: ${result.resultType} (${result.testCount} tests, ${result.successfulTestCount} passed, ${result.failedTestCount} failed, ${result.skippedTestCount} skipped)"
          def startItem = '\|  ', endItem = '  \|'
          def repeatLength = startItem.length() + output.length() + endItem.length()
          println('\\n' + ('-' * repeatLength) + '\\n' + startItem + output + endItem + '\\n' + ('-' * repeatLength))
      }
  }
}"""

    return [
        f"""
sed -i '
/testLogging {{/,/}}/{{
  /testLogging {{/r /dev/stdin
  d
}}
' {gradle_file} << 'EOF'
{new_content}
EOF
""".strip()
    ] |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_rxjava_pre_install_script

```
make_rxjava_pre_install_script() -> List[str]
```

This script modifies the gradle config to print all test results, including
passing tests.

Source code in `swebench/harness/constants/java.py` | 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150 | def make_rxjava_pre_install_script() -> List[str]:
    """
    This script modifies the gradle config to print all test results, including
    passing tests.
    """
    gradle_file = "build.gradle"

    new_content = """testLogging {
    outputs.upToDateWhen { false }
    showStandardStreams = true
    showStackTraces = true

    // Show output for all logging levels
    events = ['passed', 'skipped', 'failed', 'standardOut', 'standardError']

    // set options for log level LIFECYCLE
    events org.gradle.api.tasks.testing.logging.TestLogEvent.FAILED,
           org.gradle.api.tasks.testing.logging.TestLogEvent.PASSED,
           org.gradle.api.tasks.testing.logging.TestLogEvent.SKIPPED,
           org.gradle.api.tasks.testing.logging.TestLogEvent.STANDARD_OUT,
           org.gradle.api.tasks.testing.logging.TestLogEvent.STANDARD_ERROR
    exceptionFormat org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    showExceptions true
    showCauses true
    showStackTraces true

    // set options for log level DEBUG and INFO
    debug {
        events org.gradle.api.tasks.testing.logging.TestLogEvent.STARTED,
               org.gradle.api.tasks.testing.logging.TestLogEvent.FAILED,
               org.gradle.api.tasks.testing.logging.TestLogEvent.PASSED,
               org.gradle.api.tasks.testing.logging.TestLogEvent.SKIPPED,
               org.gradle.api.tasks.testing.logging.TestLogEvent.STANDARD_ERROR,
               org.gradle.api.tasks.testing.logging.TestLogEvent.STANDARD_OUT
        exceptionFormat org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
    info.events = debug.events
    info.exceptionFormat = debug.exceptionFormat

    afterSuite { desc, result ->
        if (!desc.parent) { // will match the outermost suite
            def output = "Results: ${result.resultType} (${result.testCount} tests, ${result.successfulTestCount} passed, ${result.failedTestCount} failed, ${result.skippedTestCount} skipped)"
            def startItem = '\|  ', endItem = '  \|'
            def repeatLength = startItem.length() + output.length() + endItem.length()
            println('\\n' + ('-' * repeatLength) + '\\n' + startItem + output + endItem + '\\n' + ('-' * repeatLength))
        }
    }
}"""

    return [
        f"""
sed -i '
/testLogging {{/,/}}/{{
  /testLogging {{/r /dev/stdin
  d
}}
' {gradle_file} << 'EOF'
{new_content}
EOF
""".strip()
    ] |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            javascript

######
            TEST_XVFB_PREFIX

  `module-attribute`

```
TEST_XVFB_PREFIX = 'xvfb-run --server-args="-screen 0 1280x1024x24 -ac :99"'
```

######
            XVFB_DEPS

  `module-attribute`

```
XVFB_DEPS = ['python3', 'python3-pip', 'xvfb', 'x11-xkb-utils', 'xfonts-100dpi', 'xfonts-75dpi', 'xfonts-scalable', 'xfonts-cyrillic', 'x11-apps', 'firefox']
```

######
            X11_DEPS

  `module-attribute`

```
X11_DEPS = ['libx11-xcb1', 'libxcomposite1', 'libxcursor1', 'libxdamage1', 'libxi6', 'libxtst6', 'libnss3', 'libcups2', 'libxss1', 'libxrandr2', 'libasound2', 'libatk1.0-0', 'libgtk-3-0', 'x11-utils']
```

######
            SPECS_CALYPSO

  `module-attribute`

```
SPECS_CALYPSO = {None: {k: {'apt-pkgs': ['libsass-dev', 'sassc'], 'install': ['npm install --unsafe-perm'], 'test_cmd': 'npm run test-client', 'docker_specs': {'node_version': k}} for k in ['0.8', '4.2.3', '4.3.0', '5.10.1', '5.11.1', '6.1.0', '6.7.0', '6.9.0', '6.9.1', '6.9.4', '6.10.0', '6.10.2', '6.10.3', '6.11.1', '6.11.2', '6.11.5', '8.9.1', '8.9.3', '8.9.4', '8.11.0', '8.11.2', '10.4.1', '10.5.0', '10.6.0', '10.9.0', '10.10.0', '10.12.0', '10.13.0', '10.14.0', '10.15.2', '10.16.3']}}
```

######
            TEST_CHART_JS_TEMPLATE

  `module-attribute`

```
TEST_CHART_JS_TEMPLATE = './node_modules/.bin/cross-env NODE_ENV=test ./node_modules/.bin/karma start {} --single-run --coverage --grep --auto-watch false'
```

######
            SPECS_CHART_JS

  `module-attribute`

```
SPECS_CHART_JS = {None: {k: {'install': ['pnpm install', 'pnpm run build'], 'test_cmd': ['pnpm install', 'pnpm run build', f'{TEST_XVFB_PREFIX} su chromeuser -c "{format('./karma.conf.cjs')}"'], 'docker_specs': {'node_version': '21.6.2', 'pnpm_version': '7.9.0', 'run_args': {'cap_add': ['SYS_ADMIN']}}} for k in ['4.0', '4.1', '4.2', '4.3', '4.4']}, None: {k: {'install': ['npm install'], 'test_cmd': ['npm install', 'npm run build', f'{TEST_XVFB_PREFIX} su chromeuser -c "{format('./karma.conf.js')}"'], 'docker_specs': {'node_version': '21.6.2', 'run_args': {'cap_add': ['SYS_ADMIN']}}} for k in ['3.0', '3.1', '3.2', '3.3', '3.4', '3.5', '3.6', '3.7', '3.8']}, None: {k: {'install': ['npm install', 'npm install -g gulp-cli'], 'test_cmd': ['npm install', 'gulp build', TEST_XVFB_PREFIX + ' su chromeuser -c "gulp test"'], 'docker_specs': {'node_version': '21.6.2', 'run_args': {'cap_add': ['SYS_ADMIN']}}} for k in ['2.0', '2.1', '2.2', '2.3', '2.4', '2.5', '2.6', '2.7', '2.8', '2.9']}}
```

######
            SPECS_MARKED

  `module-attribute`

```
SPECS_MARKED = {None: {k: {'install': ['npm install'], 'test_cmd': './node_modules/.bin/jasmine --no-color --config=jasmine.json', 'docker_specs': {'node_version': '12.22.12'}} for k in ['0.3', '0.5', '0.6', '0.7', '1.0', '1.1', '1.2', '2.0', '3.9', '4.0', '4.1', '5.0']}}
```

######
            SPECS_P5_JS

  `module-attribute`

```
SPECS_P5_JS = {None: {k: {'apt-pkgs': X11_DEPS, 'install': ['npm install', "PUPPETEER_SKIP_CHROMIUM_DOWNLOAD='' node node_modules/puppeteer/install.js", './node_modules/.bin/grunt yui'], 'test_cmd': "sed -i 's/concurrency:[[:space:]]*[0-9][0-9]*/concurrency: 1/g' Gruntfile.js\nstdbuf -o 1M ./node_modules/.bin/grunt test --quiet --force", 'docker_specs': {'node_version': '14.17.3'}} for k in ['0.10', '0.2', '0.4', '0.5', '0.6', '0.7', '0.8', '0.9', '1.0', '1.1', '1.2', '1.3', '1.4', '1.5', '1.6', '1.7', '1.8', '1.9']}}
```

######
            SPECS_REACT_PDF

  `module-attribute`

```
SPECS_REACT_PDF = {None: {k: {'apt-pkgs': ['pkg-config', 'build-essential', 'libpixman-1-0', 'libpixman-1-dev', 'libcairo2-dev', 'libpango1.0-dev', 'libjpeg-dev', 'libgif-dev', 'librsvg2-dev'] + X11_DEPS, 'install': ['npm i -g yarn', 'yarn install'], 'test_cmd': 'NODE_OPTIONS="--experimental-vm-modules" ./node_modules/.bin/jest --no-color', 'docker_specs': {'node_version': '18.20.4'}} for k in ['1.0', '1.1', '1.2', '2.0']}}
```

######
            JEST_JSON_JQ_TRANSFORM

  `module-attribute`

```
JEST_JSON_JQ_TRANSFORM = 'jq -r \'.testResults[].assertionResults[] | "[" + (.status | ascii_upcase) + "] " + ((.ancestorTitles | join(" > ")) + (if .ancestorTitles | length > 0 then " > " else "" end) + .title)\''
```

######
            SPECS_BABEL

  `module-attribute`

```
SPECS_BABEL = {'14532': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest babel-generator --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}, '13928': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest babel-parser -t "arrow" --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}, '15649': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest packages/babel-traverse/test/scope.js --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}, '15445': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest packages/babel-generator/test/index.js -t "generation " --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}, '16130': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['yarn jest babel-helpers --verbose'], 'install': ['make bootstrap'], 'build': ['make build']}}
```

######
            SPECS_VUEJS

  `module-attribute`

```
SPECS_VUEJS = {'11899': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/compiler-sfc/__tests__/compileStyle.spec.ts --no-watch --reporter=verbose'], 'install': ['pnpm i'], 'build': ['pnpm run build compiler-sfc']}, '11870': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/runtime-core/__tests__/helpers/renderList.spec.ts --no-watch --reporter=verbose'], 'install': ['pnpm i']}, '11739': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/runtime-core/__tests__/hydration.spec.ts --no-watch --reporter=verbose -t "mismatch handling"'], 'install': ['pnpm i']}, '11915': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/compiler-core/__tests__/parse.spec.ts --no-watch --reporter=verbose -t "Element"'], 'install': ['pnpm i']}, '11589': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'test_cmd': ['pnpm run test packages/runtime-core/__tests__/apiWatch.spec.ts --no-watch --reporter=verbose'], 'install': ['pnpm i']}}
```

######
            SPECS_DOCUSAURUS

  `module-attribute`

```
SPECS_DOCUSAURUS = {'10309': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus-plugin-content-docs/src/client/__tests__/docsClientUtils.test.ts --verbose']}, '10130': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus/src/server/__tests__/brokenLinks.test.ts --verbose']}, '9897': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus-utils/src/__tests__/markdownUtils.test.ts --verbose']}, '9183': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus-theme-classic/src/__tests__/options.test.ts --verbose']}, '8927': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['yarn install'], 'test_cmd': ['yarn test packages/docusaurus-utils/src/__tests__/markdownLinks.test.ts --verbose']}}
```

######
            SPECS_IMMUTABLEJS

  `module-attribute`

```
SPECS_IMMUTABLEJS = {'2006': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'build': ['npm run build'], 'test_cmd': ['npx jest __tests__/Range.ts --verbose']}, '2005': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'build': ['npm run build'], 'test_cmd': [f'npx jest __tests__/OrderedMap.ts __tests__/OrderedSet.ts --silent --json | {JEST_JSON_JQ_TRANSFORM}']}}
```

######
            SPECS_THREEJS

  `module-attribute`

```
SPECS_THREEJS = {'27395': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install --ignore-scripts'], 'test_cmd': ['npx qunit test/unit/src/math/Sphere.tests.js']}, '26589': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install --ignore-scripts'], 'test_cmd': ['npx qunit test/unit/src/objects/Line.tests.js test/unit/src/objects/Mesh.tests.js test/unit/src/objects/Points.tests.js']}, '25687': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install --ignore-scripts'], 'test_cmd': ['npx qunit test/unit/src/core/Object3D.tests.js -f "/json|clone|copy/i"']}}
```

######
            SPECS_PREACT

  `module-attribute`

```
SPECS_PREACT = {'4152': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/components.test.js"']}, '4316': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/events.test.js"']}, '4245': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/useId.test.js"']}, '4182': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/errorBoundary.test.js"']}, '4436': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/refs.test.js"']}, '3763': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/lifecycles/componentDidMount.test.js"']}, '3739': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/useState.test.js"']}, '3689': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/errorBoundary.test.js"']}, '3567': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/useEffect.test.js"']}, '3562': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="compat/test/browser/render.test.js"']}, '3454': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/svg.test.js"']}, '3345': {'docker_specs': {'node_version': '18', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="hooks/test/browser/useEffect.test.js"']}, '3062': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/render.test.js"']}, '3010': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/render.test.js"']}, '2927': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/render.test.js"']}, '2896': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="compat/test/browser/memo.test.js"']}, '2757': {'docker_specs': {'node_version': '16', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['COVERAGE=false BABEL_NO_MODULES=true npx karma start karma.conf.js --single-run --grep="test/browser/render.test.js"']}}
```

######
            SPECS_AXIOS

  `module-attribute`

```
SPECS_AXIOS = {'5892': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ["npx mocha test/unit/adapters/http.js -R tap -g 'compression'"]}, '5316': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'build': ['npm install'], 'test_cmd': ["npx mocha test/unit/adapters/http.js -R tap -g 'FormData'"]}, '4738': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ["timeout 10s npx mocha -R tap test/unit/adapters/http.js -g 'timeout'"]}, '4731': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ["npx mocha -R tap test/unit/adapters/http.js -g 'body length'"]}, '6539': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['npx mocha -R tap test/unit/regression/SNYK-JS-AXIOS-7361793.js']}, '5085': {'docker_specs': {'node_version': '20', '_variant': 'js_2'}, 'install': ['npm install'], 'test_cmd': ['npx mocha -R tap test/unit/regression/bugs.js']}}
```

######
            MAP_REPO_VERSION_TO_SPECS_JS

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_JS = {'Automattic/wp-calypso': SPECS_CALYPSO, 'chartjs/Chart.js': SPECS_CHART_JS, 'markedjs/marked': SPECS_MARKED, 'processing/p5.js': SPECS_P5_JS, 'diegomura/react-pdf': SPECS_REACT_PDF, 'babel/babel': SPECS_BABEL, 'vuejs/core': SPECS_VUEJS, 'facebook/docusaurus': SPECS_DOCUSAURUS, 'immutable-js/immutable-js': SPECS_IMMUTABLEJS, 'mrdoob/three.js': SPECS_THREEJS, 'preactjs/preact': SPECS_PREACT, 'axios/axios': SPECS_AXIOS}
```

######
            MAP_REPO_TO_INSTALL_JS

  `module-attribute`

```
MAP_REPO_TO_INSTALL_JS = {}
```

#####
            php

######
            SPECS_PHPSPREADSHEET

  `module-attribute`

```
SPECS_PHPSPREADSHEET = {'4313': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Reader/Ods/FormulaTranslatorTest.php']}, '4214': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Calculation/Functions/MathTrig/RoundDownTest.php']}, '4186': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Writer/Xlsx/FunctionPrefixTest.php']}, '4114': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Worksheet/Issue4112Test.php']}, '3940': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Worksheet/WorksheetTest.php']}, '3903': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Shared/StringHelperTest.php']}, '3570': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Calculation/Functions/LookupRef/VLookupTest.php']}, '3463': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Writer/Xlsx/FunctionPrefixTest.php']}, '3469': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Style/StyleTest.php']}, '3659': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['./vendor/bin/phpunit --testdox --colors=never tests/PhpSpreadsheetTests/Worksheet/Table/Issue3635Test.php']}}
```

######
            SPECS_LARAVEL_FRAMEWORK

  `module-attribute`

```
SPECS_LARAVEL_FRAMEWORK = {'53914': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Integration/Database/DatabaseConnectionsTest.php']}, '53206': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Support/SupportJsTest.php']}, '52866': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Container/ContextualAttributeBindingTest.php']}, '52684': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Support/SupportStrTest.php']}, '52680': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Database/DatabaseEloquentInverseRelationTest.php']}, '52451': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ["vendor/bin/phpunit --testdox --colors=never tests/Validation/ValidationValidatorTest.php --filter 'custom'"]}, '53949': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Support/OnceTest.php']}, '51890': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ["vendor/bin/phpunit --testdox --colors=never tests/Validation/ValidationValidatorTest.php --filter 'attribute'"]}, '51195': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/View/Blade/BladeVerbatimTest.php']}, '48636': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Database/DatabaseEloquentModelTest.php']}, '48573': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Cache/CacheArrayStoreTest.php']}, '46234': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require laravel/prompts --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Routing/RoutingUrlGeneratorTest.php']}, '53696': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer require orchestra/testbench-core --no-update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Database/DatabaseSchemaBlueprintTest.php']}}
```

######
            SPECS_PHP_CS_FIXER

  `module-attribute`

```
SPECS_PHP_CS_FIXER = {'8367': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Import/FullyQualifiedStrictTypesFixerTest.php']}, '8331': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/LanguageConstruct/NullableTypeDeclarationFixerTest.php']}, '8075': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/PhpUnit/PhpUnitAttributesFixerTest.php']}, '8064': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/StringNotation/SimpleToComplexStringVariableFixerTest.php']}, '7998': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Casing/ConstantCaseFixerTest.php']}, '7875': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Whitespace/StatementIndentationFixerTest.php']}, '7635': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Import/FullyQualifiedStrictTypesFixerTest.php']}, '7523': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Operator/BinaryOperatorSpacesFixerTest.php']}, '8256': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/PhpTag/BlankLineAfterOpeningTagFixerTest.php']}, '7663': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Fixer/Whitespace/StatementIndentationFixerTest.php']}}
```

######
            SPECS_CARBON

  `module-attribute`

```
SPECS_CARBON = {'3103': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonImmutable/SettersTest.php']}, '3098': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/ConstructTest.php']}, '3073': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/TotalTest.php']}, '3041': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonPeriod/CreateTest.php']}, '3005': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/ConstructTest.php']}, '2981': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/TotalTest.php']}, '2813': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'build': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Factory/FactoryTest.php']}, '2752': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonImmutable/IsTest.php']}, '2665': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/Carbon/RoundTest.php']}, '2762': {'docker_specs': {'php_version': '8.3.16'}, 'install': ['composer update', 'composer install'], 'test_cmd': ['vendor/bin/phpunit --testdox --colors=never tests/CarbonInterval/RoundingTest.php']}}
```

######
            MAP_REPO_VERSION_TO_SPECS_PHP

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_PHP = {'phpoffice/phpspreadsheet': SPECS_PHPSPREADSHEET, 'laravel/framework': SPECS_LARAVEL_FRAMEWORK, 'php-cs-fixer/php-cs-fixer': SPECS_PHP_CS_FIXER, 'briannesbitt/carbon': SPECS_CARBON}
```

######
            MAP_REPO_TO_INSTALL_PHP

  `module-attribute`

```
MAP_REPO_TO_INSTALL_PHP = {}
```

#####
            python

######
            TEST_ASTROPY_PYTEST

  `module-attribute`

```
TEST_ASTROPY_PYTEST = 'pytest -rA -vv -o console_output_style=classic --tb=no'
```

######
            TEST_DJANGO

  `module-attribute`

```
TEST_DJANGO = './tests/runtests.py --verbosity 2 --settings=test_sqlite --parallel 1'
```

######
            TEST_DJANGO_NO_PARALLEL

  `module-attribute`

```
TEST_DJANGO_NO_PARALLEL = './tests/runtests.py --verbosity 2'
```

######
            TEST_SEABORN

  `module-attribute`

```
TEST_SEABORN = 'pytest --no-header -rA'
```

######
            TEST_SEABORN_VERBOSE

  `module-attribute`

```
TEST_SEABORN_VERBOSE = 'pytest -rA --tb=long'
```

######
            TEST_PYTEST

  `module-attribute`

```
TEST_PYTEST = 'pytest -rA'
```

######
            TEST_PYTEST_VERBOSE

  `module-attribute`

```
TEST_PYTEST_VERBOSE = 'pytest -rA --tb=long'
```

######
            TEST_SPHINX

  `module-attribute`

```
TEST_SPHINX = 'tox --current-env -epy39 -v --'
```

######
            TEST_SYMPY

  `module-attribute`

```
TEST_SYMPY = "PYTHONWARNINGS='ignore::UserWarning,ignore::SyntaxWarning' bin/test -C --verbose"
```

######
            TEST_SYMPY_VERBOSE

  `module-attribute`

```
TEST_SYMPY_VERBOSE = 'bin/test -C --verbose'
```

######
            SPECS_SKLEARN

  `module-attribute`

```
SPECS_SKLEARN = {k: {'python': '3.6', 'packages': 'numpy scipy cython pytest pandas matplotlib', 'install': 'python -m pip install -v --no-use-pep517 --no-build-isolation -e .', 'pip_packages': ['cython', 'numpy==1.19.2', 'setuptools', 'scipy==1.5.2'], 'test_cmd': TEST_PYTEST} for k in ['0.20', '0.21', '0.22']}
```

######
            SPECS_FLASK

  `module-attribute`

```
SPECS_FLASK = {'2.0': {'python': '3.9', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .', 'pip_packages': ['setuptools==70.0.0', 'Werkzeug==2.3.7', 'Jinja2==3.0.1', 'itsdangerous==2.1.2', 'click==8.0.1', 'MarkupSafe==2.1.3'], 'test_cmd': TEST_PYTEST}, '2.1': {'python': '3.10', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .', 'pip_packages': ['setuptools==70.0.0', 'click==8.1.3', 'itsdangerous==2.1.2', 'Jinja2==3.1.2', 'MarkupSafe==2.1.1', 'Werkzeug==2.3.7'], 'test_cmd': TEST_PYTEST}}
```

######
            SPECS_DJANGO

  `module-attribute`

```
SPECS_DJANGO = {k: {'python': '3.5', 'packages': 'requirements.txt', 'pre_install': ['apt-get update && apt-get install -y locales', "echo 'en_US UTF-8' > /etc/locale.gen", 'locale-gen en_US.UTF-8'], 'install': 'python setup.py install', 'pip_packages': ['setuptools'], 'eval_commands': ['export LANG=en_US.UTF-8', 'export LC_ALL=en_US.UTF-8', 'export PYTHONIOENCODING=utf8', 'export LANGUAGE=en_US:en'], 'test_cmd': TEST_DJANGO} for k in ['1.7', '1.8', '1.9', '1.10', '1.11', '2.0', '2.1', '2.2']}
```

######
            SPECS_REQUESTS

  `module-attribute`

```
SPECS_REQUESTS = {k: {'python': '3.9', 'packages': 'pytest', 'install': 'python -m pip install .', 'test_cmd': TEST_PYTEST} for k in (['0.7', '0.8', '0.9', '0.11', '0.13', '0.14', '1.1', '1.2', '2.0', '2.2'] + ['2.3', '2.4', '2.5', '2.7', '2.8', '2.9', '2.10', '2.11', '2.12', '2.17'] + ['2.18', '2.19', '2.22', '2.26', '2.25', '2.27', '2.31', '3.0'])}
```

######
            SPECS_SEABORN

  `module-attribute`

```
SPECS_SEABORN = {k: {'python': '3.9', 'install': 'python -m pip install -e .', 'pip_packages': ['contourpy==1.1.0', 'cycler==0.11.0', 'fonttools==4.42.1', 'importlib-resources==6.0.1', 'kiwisolver==1.4.5', 'matplotlib==3.7.2', 'numpy==1.25.2', 'packaging==23.1', 'pandas==1.3.5', 'pillow==10.0.0', 'pyparsing==3.0.9', 'pytest', 'python-dateutil==2.8.2', 'pytz==2023.3.post1', 'scipy==1.11.2', 'six==1.16.0', 'tzdata==2023.1', 'zipp==3.16.2'], 'test_cmd': TEST_SEABORN} for k in ['0.11']}
```

######
            SPECS_PYTEST

  `module-attribute`

```
SPECS_PYTEST = {k: {'python': '3.9', 'install': 'python -m pip install -e .', 'test_cmd': TEST_PYTEST} for k in ['4.4', '4.5', '4.6', '5.0', '5.1', '5.2', '5.3', '5.4', '6.0', '6.2', '6.3', '7.0', '7.1', '7.2', '7.4', '8.0', '8.1', '8.2', '8.3', '8.4']}
```

######
            SPECS_MATPLOTLIB

  `module-attribute`

```
SPECS_MATPLOTLIB = {k: {'python': '3.11', 'packages': 'environment.yml', 'install': 'python -m pip install -e .', 'pre_install': ['apt-get -y update && apt-get -y upgrade && DEBIAN_FRONTEND=noninteractive apt-get install -y imagemagick ffmpeg texlive texlive-latex-extra texlive-fonts-recommended texlive-xetex texlive-luatex cm-super dvipng', 'QHULL_URL="http://www.qhull.org/download/qhull-2020-src-8.0.2.tgz"', 'QHULL_TAR="/tmp/qhull-2020-src-8.0.2.tgz"', 'QHULL_BUILD_DIR="/testbed/build"', 'wget -O "$QHULL_TAR" "$QHULL_URL"', 'mkdir -p "$QHULL_BUILD_DIR"', 'tar -xvzf "$QHULL_TAR" -C "$QHULL_BUILD_DIR"'], 'pip_packages': ['contourpy==1.1.0', 'cycler==0.11.0', 'fonttools==4.42.1', 'ghostscript', 'kiwisolver==1.4.5', 'numpy==1.25.2', 'packaging==23.1', 'pillow==10.0.0', 'pikepdf', 'pyparsing==3.0.9', 'python-dateutil==2.8.2', 'six==1.16.0', 'setuptools==68.1.2', 'setuptools-scm==7.1.0', 'typing-extensions==4.7.1'], 'test_cmd': TEST_PYTEST} for k in ['3.5', '3.6', '3.7', '3.8', '3.9']}
```

######
            SPECS_SPHINX

  `module-attribute`

```
SPECS_SPHINX = {k: {'python': '3.9', 'pip_packages': ['tox==4.16.0', 'tox-current-env==0.0.11', 'Jinja2==3.0.3'], 'install': 'python -m pip install -e .[test]', 'pre_install': ["sed -i 's/pytest/pytest -rA/' tox.ini"], 'test_cmd': TEST_SPHINX} for k in (['1.5', '1.6', '1.7', '1.8', '2.0', '2.1', '2.2', '2.3', '2.4', '3.0'] + ['3.1', '3.2', '3.3', '3.4', '3.5', '4.0', '4.1', '4.2', '4.3', '4.4'] + ['4.5', '5.0', '5.1', '5.2', '5.3', '6.0', '6.2', '7.0', '7.1', '7.2'] + ['7.3', '7.4', '8.0', '8.1'])}
```

######
            SPECS_ASTROPY

  `module-attribute`

```
SPECS_ASTROPY = {k: {'python': '3.9', 'install': 'python -m pip install -e .[test] --verbose', 'pip_packages': ['attrs==23.1.0', 'exceptiongroup==1.1.3', 'execnet==2.0.2', 'hypothesis==6.82.6', 'iniconfig==2.0.0', 'numpy==1.25.2', 'packaging==23.1', 'pluggy==1.3.0', 'psutil==5.9.5', 'pyerfa==2.0.0.3', 'pytest-arraydiff==0.5.0', 'pytest-astropy-header==0.2.2', 'pytest-astropy==0.10.0', 'pytest-cov==4.1.0', 'pytest-doctestplus==1.0.0', 'pytest-filter-subpackage==0.1.2', 'pytest-mock==3.11.1', 'pytest-openfiles==0.5.0', 'pytest-remotedata==0.4.0', 'pytest-xdist==3.3.1', 'pytest==7.4.0', 'PyYAML==6.0.1', 'setuptools==68.0.0', 'sortedcontainers==2.4.0', 'tomli==2.0.1'], 'test_cmd': TEST_PYTEST} for k in ['3.0', '3.1', '3.2', '4.1', '4.2', '4.3', '5.0', '5.1', '5.2', 'v5.3']}
```

######
            SPECS_SYMPY

  `module-attribute`

```
SPECS_SYMPY = {k: {'python': '3.9', 'packages': 'mpmath flake8', 'pip_packages': ['mpmath==1.3.0', 'flake8-comprehensions'], 'install': 'python -m pip install -e .', 'test_cmd': TEST_SYMPY} for k in (['0.7', '1.0', '1.1', '1.10', '1.11', '1.12', '1.2', '1.4', '1.5', '1.6'] + ['1.7', '1.8', '1.9'] + ['1.10', '1.11', '1.12', '1.13', '1.14'])}
```

######
            SPECS_PYLINT

  `module-attribute`

```
SPECS_PYLINT = {k: {'python': '3.9', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .', 'test_cmd': TEST_PYTEST} for k in ['2.10', '2.11', '2.13', '2.14', '2.15', '2.16', '2.17', '2.8', '2.9', '3.0', '3.1', '3.2', '3.3', '4.0']}
```

######
            SPECS_XARRAY

  `module-attribute`

```
SPECS_XARRAY = {k: {'python': '3.10', 'packages': 'environment.yml', 'install': 'python -m pip install -e .', 'pip_packages': ['numpy==1.23.0', 'packaging==23.1', 'pandas==1.5.3', 'pytest==7.4.0', 'python-dateutil==2.8.2', 'pytz==2023.3', 'six==1.16.0', 'scipy==1.11.1', 'setuptools==68.0.0', 'dask==2022.8.1'], 'no_use_env': True, 'test_cmd': TEST_PYTEST} for k in ['0.12', '0.18', '0.19', '0.20', '2022.03', '2022.06', '2022.09', '2023.07', '2024.05']}
```

######
            SPECS_SQLFLUFF

  `module-attribute`

```
SPECS_SQLFLUFF = {k: {'python': '3.9', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .', 'test_cmd': TEST_PYTEST} for k in ['0.10', '0.11', '0.12', '0.13', '0.4', '0.5', '0.6', '0.8', '0.9', '1.0', '1.1', '1.2', '1.3', '1.4', '2.0', '2.1', '2.2']}
```

######
            SPECS_DBT_CORE

  `module-attribute`

```
SPECS_DBT_CORE = {k: {'python': '3.9', 'packages': 'requirements.txt', 'install': 'python -m pip install -e .'} for k in ['0.13', '0.14', '0.15', '0.16', '0.17', '0.18', '0.19', '0.20', '0.21', '1.0', '1.1', '1.2', '1.3', '1.4', '1.5', '1.6', '1.7']}
```

######
            SPECS_PYVISTA

  `module-attribute`

```
SPECS_PYVISTA = {k: {'python': '3.9', 'install': 'python -m pip install -e .', 'pip_packages': ['pytest'], 'test_cmd': TEST_PYTEST} for k in ['0.20', '0.21', '0.22', '0.23']}
```

######
            SPECS_ASTROID

  `module-attribute`

```
SPECS_ASTROID = {k: {'python': '3.9', 'install': 'python -m pip install -e .', 'pip_packages': ['pytest'], 'test_cmd': TEST_PYTEST} for k in ['2.10', '2.12', '2.13', '2.14', '2.15', '2.16', '2.5', '2.6', '2.7', '2.8', '2.9', '3.0']}
```

######
            SPECS_MARSHMALLOW

  `module-attribute`

```
SPECS_MARSHMALLOW = {k: {'python': '3.9', 'install': "python -m pip install -e '.[dev]'", 'test_cmd': TEST_PYTEST} for k in ['2.18', '2.19', '2.20', '3.0', '3.1', '3.10', '3.11', '3.12', '3.13', '3.15', '3.16', '3.19', '3.2', '3.4', '3.8', '3.9']}
```

######
            SPECS_PVLIB

  `module-attribute`

```
SPECS_PVLIB = {k: {'python': '3.9', 'install': 'python -m pip install -e .[all]', 'packages': 'pandas scipy', 'pip_packages': ['jupyter', 'ipython', 'matplotlib', 'pytest', 'flake8'], 'test_cmd': TEST_PYTEST} for k in ['0.1', '0.2', '0.3', '0.4', '0.5', '0.6', '0.7', '0.8', '0.9']}
```

######
            SPECS_PYDICOM

  `module-attribute`

```
SPECS_PYDICOM = {k: {'python': '3.6', 'install': 'python -m pip install -e .', 'packages': 'numpy', 'pip_packages': ['pytest'], 'test_cmd': TEST_PYTEST} for k in ['1.0', '1.1', '1.2', '1.3', '1.4', '2.0', '2.1', '2.2', '2.3', '2.4', '3.0']}
```

######
            SPECS_HUMANEVAL

  `module-attribute`

```
SPECS_HUMANEVAL = {k: {'python': '3.9', 'test_cmd': 'python'} for k in ['1.0']}
```

######
            MAP_REPO_VERSION_TO_SPECS_PY

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_PY = {'astropy/astropy': SPECS_ASTROPY, 'dbt-labs/dbt-core': SPECS_DBT_CORE, 'django/django': SPECS_DJANGO, 'matplotlib/matplotlib': SPECS_MATPLOTLIB, 'marshmallow-code/marshmallow': SPECS_MARSHMALLOW, 'mwaskom/seaborn': SPECS_SEABORN, 'pallets/flask': SPECS_FLASK, 'psf/requests': SPECS_REQUESTS, 'pvlib/pvlib-python': SPECS_PVLIB, 'pydata/xarray': SPECS_XARRAY, 'pydicom/pydicom': SPECS_PYDICOM, 'pylint-dev/astroid': SPECS_ASTROID, 'pylint-dev/pylint': SPECS_PYLINT, 'pytest-dev/pytest': SPECS_PYTEST, 'pyvista/pyvista': SPECS_PYVISTA, 'scikit-learn/scikit-learn': SPECS_SKLEARN, 'sphinx-doc/sphinx': SPECS_SPHINX, 'sqlfluff/sqlfluff': SPECS_SQLFLUFF, 'swe-bench/humaneval': SPECS_HUMANEVAL, 'sympy/sympy': SPECS_SYMPY}
```

######
            MAP_REPO_TO_INSTALL_PY

  `module-attribute`

```
MAP_REPO_TO_INSTALL_PY = {}
```

######
            MAP_REPO_TO_REQS_PATHS

  `module-attribute`

```
MAP_REPO_TO_REQS_PATHS = {'dbt-labs/dbt-core': ['dev-requirements.txt', 'dev_requirements.txt'], 'django/django': ['tests/requirements/py3.txt'], 'matplotlib/matplotlib': ['requirements/dev/dev-requirements.txt', 'requirements/testing/travis_all.txt'], 'pallets/flask': ['requirements/dev.txt'], 'pylint-dev/pylint': ['requirements_test.txt'], 'pyvista/pyvista': ['requirements_test.txt', 'requirements.txt'], 'sqlfluff/sqlfluff': ['requirements_dev.txt'], 'sympy/sympy': ['requirements-dev.txt', 'requirements-test.txt']}
```

######
            MAP_REPO_TO_ENV_YML_PATHS

  `module-attribute`

```
MAP_REPO_TO_ENV_YML_PATHS = {'matplotlib/matplotlib': ['environment.yml'], 'pydata/xarray': ['ci/requirements/environment.yml', 'environment.yml']}
```

######
            USE_X86_PY

  `module-attribute`

```
USE_X86_PY = {'astropy__astropy-7973', 'django__django-10087', 'django__django-10097', 'django__django-10213', 'django__django-10301', 'django__django-10316', 'django__django-10426', 'django__django-11383', 'django__django-12185', 'django__django-12497', 'django__django-13121', 'django__django-13417', 'django__django-13431', 'django__django-13447', 'django__django-14155', 'django__django-14164', 'django__django-14169', 'django__django-14170', 'django__django-15180', 'django__django-15199', 'django__django-15280', 'django__django-15292', 'django__django-15474', 'django__django-15682', 'django__django-15689', 'django__django-15695', 'django__django-15698', 'django__django-15781', 'django__django-15925', 'django__django-15930', 'django__django-5158', 'django__django-5470', 'django__django-7188', 'django__django-7475', 'django__django-7530', 'django__django-8326', 'django__django-8961', 'django__django-9003', 'django__django-9703', 'django__django-9871', 'matplotlib__matplotlib-13983', 'matplotlib__matplotlib-13984', 'matplotlib__matplotlib-13989', 'matplotlib__matplotlib-14043', 'matplotlib__matplotlib-14471', 'matplotlib__matplotlib-22711', 'matplotlib__matplotlib-22719', 'matplotlib__matplotlib-22734', 'matplotlib__matplotlib-22767', 'matplotlib__matplotlib-22815', 'matplotlib__matplotlib-22835', 'matplotlib__matplotlib-22865', 'matplotlib__matplotlib-22871', 'matplotlib__matplotlib-22883', 'matplotlib__matplotlib-22926', 'matplotlib__matplotlib-22929', 'matplotlib__matplotlib-22931', 'matplotlib__matplotlib-22945', 'matplotlib__matplotlib-22991', 'matplotlib__matplotlib-23031', 'matplotlib__matplotlib-23047', 'matplotlib__matplotlib-23049', 'matplotlib__matplotlib-23057', 'matplotlib__matplotlib-23088', 'matplotlib__matplotlib-23111', 'matplotlib__matplotlib-23140', 'matplotlib__matplotlib-23174', 'matplotlib__matplotlib-23188', 'matplotlib__matplotlib-23198', 'matplotlib__matplotlib-23203', 'matplotlib__matplotlib-23266', 'matplotlib__matplotlib-23267', 'matplotlib__matplotlib-23288', 'matplotlib__matplotlib-23299', 'matplotlib__matplotlib-23314', 'matplotlib__matplotlib-23332', 'matplotlib__matplotlib-23348', 'matplotlib__matplotlib-23412', 'matplotlib__matplotlib-23476', 'matplotlib__matplotlib-23516', 'matplotlib__matplotlib-23562', 'matplotlib__matplotlib-23563', 'matplotlib__matplotlib-23573', 'matplotlib__matplotlib-23740', 'matplotlib__matplotlib-23742', 'matplotlib__matplotlib-23913', 'matplotlib__matplotlib-23964', 'matplotlib__matplotlib-23987', 'matplotlib__matplotlib-24013', 'matplotlib__matplotlib-24026', 'matplotlib__matplotlib-24088', 'matplotlib__matplotlib-24111', 'matplotlib__matplotlib-24149', 'matplotlib__matplotlib-24177', 'matplotlib__matplotlib-24189', 'matplotlib__matplotlib-24224', 'matplotlib__matplotlib-24250', 'matplotlib__matplotlib-24257', 'matplotlib__matplotlib-24265', 'matplotlib__matplotlib-24334', 'matplotlib__matplotlib-24362', 'matplotlib__matplotlib-24403', 'matplotlib__matplotlib-24431', 'matplotlib__matplotlib-24538', 'matplotlib__matplotlib-24570', 'matplotlib__matplotlib-24604', 'matplotlib__matplotlib-24619', 'matplotlib__matplotlib-24627', 'matplotlib__matplotlib-24637', 'matplotlib__matplotlib-24691', 'matplotlib__matplotlib-24749', 'matplotlib__matplotlib-24768', 'matplotlib__matplotlib-24849', 'matplotlib__matplotlib-24870', 'matplotlib__matplotlib-24912', 'matplotlib__matplotlib-24924', 'matplotlib__matplotlib-24970', 'matplotlib__matplotlib-24971', 'matplotlib__matplotlib-25027', 'matplotlib__matplotlib-25052', 'matplotlib__matplotlib-25079', 'matplotlib__matplotlib-25085', 'matplotlib__matplotlib-25122', 'matplotlib__matplotlib-25126', 'matplotlib__matplotlib-25129', 'matplotlib__matplotlib-25238', 'matplotlib__matplotlib-25281', 'matplotlib__matplotlib-25287', 'matplotlib__matplotlib-25311', 'matplotlib__matplotlib-25332', 'matplotlib__matplotlib-25334', 'matplotlib__matplotlib-25340', 'matplotlib__matplotlib-25346', 'matplotlib__matplotlib-25404', 'matplotlib__matplotlib-25405', 'matplotlib__matplotlib-25425', 'matplotlib__matplotlib-25430', 'matplotlib__matplotlib-25433', 'matplotlib__matplotlib-25442', 'matplotlib__matplotlib-25479', 'matplotlib__matplotlib-25498', 'matplotlib__matplotlib-25499', 'matplotlib__matplotlib-25515', 'matplotlib__matplotlib-25547', 'matplotlib__matplotlib-25551', 'matplotlib__matplotlib-25565', 'matplotlib__matplotlib-25624', 'matplotlib__matplotlib-25631', 'matplotlib__matplotlib-25640', 'matplotlib__matplotlib-25651', 'matplotlib__matplotlib-25667', 'matplotlib__matplotlib-25712', 'matplotlib__matplotlib-25746', 'matplotlib__matplotlib-25772', 'matplotlib__matplotlib-25775', 'matplotlib__matplotlib-25779', 'matplotlib__matplotlib-25785', 'matplotlib__matplotlib-25794', 'matplotlib__matplotlib-25859', 'matplotlib__matplotlib-25960', 'matplotlib__matplotlib-26011', 'matplotlib__matplotlib-26020', 'matplotlib__matplotlib-26024', 'matplotlib__matplotlib-26078', 'matplotlib__matplotlib-26089', 'matplotlib__matplotlib-26101', 'matplotlib__matplotlib-26113', 'matplotlib__matplotlib-26122', 'matplotlib__matplotlib-26160', 'matplotlib__matplotlib-26184', 'matplotlib__matplotlib-26208', 'matplotlib__matplotlib-26223', 'matplotlib__matplotlib-26232', 'matplotlib__matplotlib-26249', 'matplotlib__matplotlib-26278', 'matplotlib__matplotlib-26285', 'matplotlib__matplotlib-26291', 'matplotlib__matplotlib-26300', 'matplotlib__matplotlib-26311', 'matplotlib__matplotlib-26341', 'matplotlib__matplotlib-26342', 'matplotlib__matplotlib-26399', 'matplotlib__matplotlib-26466', 'matplotlib__matplotlib-26469', 'matplotlib__matplotlib-26472', 'matplotlib__matplotlib-26479', 'matplotlib__matplotlib-26532', 'pydata__xarray-2905', 'pydata__xarray-2922', 'pydata__xarray-3095', 'pydata__xarray-3114', 'pydata__xarray-3151', 'pydata__xarray-3156', 'pydata__xarray-3159', 'pydata__xarray-3239', 'pydata__xarray-3302', 'pydata__xarray-3305', 'pydata__xarray-3338', 'pydata__xarray-3364', 'pydata__xarray-3406', 'pydata__xarray-3520', 'pydata__xarray-3527', 'pydata__xarray-3631', 'pydata__xarray-3635', 'pydata__xarray-3637', 'pydata__xarray-3649', 'pydata__xarray-3677', 'pydata__xarray-3733', 'pydata__xarray-3812', 'pydata__xarray-3905', 'pydata__xarray-3976', 'pydata__xarray-3979', 'pydata__xarray-3993', 'pydata__xarray-4075', 'pydata__xarray-4094', 'pydata__xarray-4098', 'pydata__xarray-4182', 'pydata__xarray-4184', 'pydata__xarray-4248', 'pydata__xarray-4339', 'pydata__xarray-4356', 'pydata__xarray-4419', 'pydata__xarray-4423', 'pydata__xarray-4442', 'pydata__xarray-4493', 'pydata__xarray-4510', 'pydata__xarray-4629', 'pydata__xarray-4683', 'pydata__xarray-4684', 'pydata__xarray-4687', 'pydata__xarray-4695', 'pydata__xarray-4750', 'pydata__xarray-4758', 'pydata__xarray-4759', 'pydata__xarray-4767', 'pydata__xarray-4802', 'pydata__xarray-4819', 'pydata__xarray-4827', 'pydata__xarray-4879', 'pydata__xarray-4911', 'pydata__xarray-4939', 'pydata__xarray-4940', 'pydata__xarray-4966', 'pydata__xarray-4994', 'pydata__xarray-5033', 'pydata__xarray-5126', 'pydata__xarray-5131', 'pydata__xarray-5180', 'pydata__xarray-5187', 'pydata__xarray-5233', 'pydata__xarray-5362', 'pydata__xarray-5365', 'pydata__xarray-5455', 'pydata__xarray-5580', 'pydata__xarray-5662', 'pydata__xarray-5682', 'pydata__xarray-5731', 'pydata__xarray-6135', 'pydata__xarray-6386', 'pydata__xarray-6394', 'pydata__xarray-6400', 'pydata__xarray-6461', 'pydata__xarray-6548', 'pydata__xarray-6598', 'pydata__xarray-6599', 'pydata__xarray-6601', 'pydata__xarray-6721', 'pydata__xarray-6744', 'pydata__xarray-6798', 'pydata__xarray-6804', 'pydata__xarray-6823', 'pydata__xarray-6857', 'pydata__xarray-6882', 'pydata__xarray-6889', 'pydata__xarray-6938', 'pydata__xarray-6971', 'pydata__xarray-6992', 'pydata__xarray-6999', 'pydata__xarray-7003', 'pydata__xarray-7019', 'pydata__xarray-7052', 'pydata__xarray-7089', 'pydata__xarray-7101', 'pydata__xarray-7105', 'pydata__xarray-7112', 'pydata__xarray-7120', 'pydata__xarray-7147', 'pydata__xarray-7150', 'pydata__xarray-7179', 'pydata__xarray-7203', 'pydata__xarray-7229', 'pydata__xarray-7233', 'pydata__xarray-7347', 'pydata__xarray-7391', 'pydata__xarray-7393', 'pydata__xarray-7400', 'pydata__xarray-7444', 'pytest-dev__pytest-10482', 'scikit-learn__scikit-learn-10198', 'scikit-learn__scikit-learn-10297', 'scikit-learn__scikit-learn-10306', 'scikit-learn__scikit-learn-10331', 'scikit-learn__scikit-learn-10377', 'scikit-learn__scikit-learn-10382', 'scikit-learn__scikit-learn-10397', 'scikit-learn__scikit-learn-10427', 'scikit-learn__scikit-learn-10428', 'scikit-learn__scikit-learn-10443', 'scikit-learn__scikit-learn-10452', 'scikit-learn__scikit-learn-10459', 'scikit-learn__scikit-learn-10471', 'scikit-learn__scikit-learn-10483', 'scikit-learn__scikit-learn-10495', 'scikit-learn__scikit-learn-10508', 'scikit-learn__scikit-learn-10558', 'scikit-learn__scikit-learn-10577', 'scikit-learn__scikit-learn-10581', 'scikit-learn__scikit-learn-10687', 'scikit-learn__scikit-learn-10774', 'scikit-learn__scikit-learn-10777', 'scikit-learn__scikit-learn-10803', 'scikit-learn__scikit-learn-10844', 'scikit-learn__scikit-learn-10870', 'scikit-learn__scikit-learn-10881', 'scikit-learn__scikit-learn-10899', 'scikit-learn__scikit-learn-10908', 'scikit-learn__scikit-learn-10913', 'scikit-learn__scikit-learn-10949', 'scikit-learn__scikit-learn-10982', 'scikit-learn__scikit-learn-10986', 'scikit-learn__scikit-learn-11040', 'scikit-learn__scikit-learn-11042', 'scikit-learn__scikit-learn-11043', 'scikit-learn__scikit-learn-11151', 'scikit-learn__scikit-learn-11160', 'scikit-learn__scikit-learn-11206', 'scikit-learn__scikit-learn-11235', 'scikit-learn__scikit-learn-11243', 'scikit-learn__scikit-learn-11264', 'scikit-learn__scikit-learn-11281', 'scikit-learn__scikit-learn-11310', 'scikit-learn__scikit-learn-11315', 'scikit-learn__scikit-learn-11333', 'scikit-learn__scikit-learn-11346', 'scikit-learn__scikit-learn-11391', 'scikit-learn__scikit-learn-11496', 'scikit-learn__scikit-learn-11542', 'scikit-learn__scikit-learn-11574', 'scikit-learn__scikit-learn-11578', 'scikit-learn__scikit-learn-11585', 'scikit-learn__scikit-learn-11596', 'scikit-learn__scikit-learn-11635', 'scikit-learn__scikit-learn-12258', 'scikit-learn__scikit-learn-12421', 'scikit-learn__scikit-learn-12443', 'scikit-learn__scikit-learn-12462', 'scikit-learn__scikit-learn-12471', 'scikit-learn__scikit-learn-12486', 'scikit-learn__scikit-learn-12557', 'scikit-learn__scikit-learn-12583', 'scikit-learn__scikit-learn-12585', 'scikit-learn__scikit-learn-12625', 'scikit-learn__scikit-learn-12626', 'scikit-learn__scikit-learn-12656', 'scikit-learn__scikit-learn-12682', 'scikit-learn__scikit-learn-12704', 'scikit-learn__scikit-learn-12733', 'scikit-learn__scikit-learn-12758', 'scikit-learn__scikit-learn-12760', 'scikit-learn__scikit-learn-12784', 'scikit-learn__scikit-learn-12827', 'scikit-learn__scikit-learn-12834', 'scikit-learn__scikit-learn-12860', 'scikit-learn__scikit-learn-12908', 'scikit-learn__scikit-learn-12938', 'scikit-learn__scikit-learn-12961', 'scikit-learn__scikit-learn-12973', 'scikit-learn__scikit-learn-12983', 'scikit-learn__scikit-learn-12989', 'scikit-learn__scikit-learn-13010', 'scikit-learn__scikit-learn-13013', 'scikit-learn__scikit-learn-13017', 'scikit-learn__scikit-learn-13046', 'scikit-learn__scikit-learn-13087', 'scikit-learn__scikit-learn-13124', 'scikit-learn__scikit-learn-13135', 'scikit-learn__scikit-learn-13142', 'scikit-learn__scikit-learn-13143', 'scikit-learn__scikit-learn-13157', 'scikit-learn__scikit-learn-13165', 'scikit-learn__scikit-learn-13174', 'scikit-learn__scikit-learn-13221', 'scikit-learn__scikit-learn-13241', 'scikit-learn__scikit-learn-13253', 'scikit-learn__scikit-learn-13280', 'scikit-learn__scikit-learn-13283', 'scikit-learn__scikit-learn-13302', 'scikit-learn__scikit-learn-13313', 'scikit-learn__scikit-learn-13328', 'scikit-learn__scikit-learn-13333', 'scikit-learn__scikit-learn-13363', 'scikit-learn__scikit-learn-13368', 'scikit-learn__scikit-learn-13392', 'scikit-learn__scikit-learn-13436', 'scikit-learn__scikit-learn-13439', 'scikit-learn__scikit-learn-13447', 'scikit-learn__scikit-learn-13454', 'scikit-learn__scikit-learn-13467', 'scikit-learn__scikit-learn-13472', 'scikit-learn__scikit-learn-13485', 'scikit-learn__scikit-learn-13496', 'scikit-learn__scikit-learn-13497', 'scikit-learn__scikit-learn-13536', 'scikit-learn__scikit-learn-13549', 'scikit-learn__scikit-learn-13554', 'scikit-learn__scikit-learn-13584', 'scikit-learn__scikit-learn-13618', 'scikit-learn__scikit-learn-13620', 'scikit-learn__scikit-learn-13628', 'scikit-learn__scikit-learn-13641', 'scikit-learn__scikit-learn-13704', 'scikit-learn__scikit-learn-13726', 'scikit-learn__scikit-learn-13779', 'scikit-learn__scikit-learn-13780', 'scikit-learn__scikit-learn-13828', 'scikit-learn__scikit-learn-13864', 'scikit-learn__scikit-learn-13877', 'scikit-learn__scikit-learn-13910', 'scikit-learn__scikit-learn-13915', 'scikit-learn__scikit-learn-13933', 'scikit-learn__scikit-learn-13960', 'scikit-learn__scikit-learn-13974', 'scikit-learn__scikit-learn-13983', 'scikit-learn__scikit-learn-14012', 'scikit-learn__scikit-learn-14024', 'scikit-learn__scikit-learn-14053', 'scikit-learn__scikit-learn-14067', 'scikit-learn__scikit-learn-14087', 'scikit-learn__scikit-learn-14092', 'scikit-learn__scikit-learn-14114', 'scikit-learn__scikit-learn-14125', 'scikit-learn__scikit-learn-14141', 'scikit-learn__scikit-learn-14237', 'scikit-learn__scikit-learn-14309', 'scikit-learn__scikit-learn-14430', 'scikit-learn__scikit-learn-14450', 'scikit-learn__scikit-learn-14458', 'scikit-learn__scikit-learn-14464', 'scikit-learn__scikit-learn-14496', 'scikit-learn__scikit-learn-14520', 'scikit-learn__scikit-learn-14544', 'scikit-learn__scikit-learn-14591', 'scikit-learn__scikit-learn-14629', 'scikit-learn__scikit-learn-14704', 'scikit-learn__scikit-learn-14706', 'scikit-learn__scikit-learn-14710', 'scikit-learn__scikit-learn-14732', 'scikit-learn__scikit-learn-14764', 'scikit-learn__scikit-learn-14806', 'scikit-learn__scikit-learn-14869', 'scikit-learn__scikit-learn-14878', 'scikit-learn__scikit-learn-14890', 'scikit-learn__scikit-learn-14894', 'scikit-learn__scikit-learn-14898', 'scikit-learn__scikit-learn-14908', 'scikit-learn__scikit-learn-14983', 'scikit-learn__scikit-learn-14999', 'scikit-learn__scikit-learn-15028', 'scikit-learn__scikit-learn-15084', 'scikit-learn__scikit-learn-15086', 'scikit-learn__scikit-learn-15094', 'scikit-learn__scikit-learn-15096', 'scikit-learn__scikit-learn-15100', 'scikit-learn__scikit-learn-15119', 'scikit-learn__scikit-learn-15120', 'scikit-learn__scikit-learn-15138', 'scikit-learn__scikit-learn-15393', 'scikit-learn__scikit-learn-15495', 'scikit-learn__scikit-learn-15512', 'scikit-learn__scikit-learn-15524', 'scikit-learn__scikit-learn-15535', 'scikit-learn__scikit-learn-15625', 'scikit-learn__scikit-learn-3840', 'scikit-learn__scikit-learn-7760', 'scikit-learn__scikit-learn-8554', 'scikit-learn__scikit-learn-9274', 'scikit-learn__scikit-learn-9288', 'scikit-learn__scikit-learn-9304', 'scikit-learn__scikit-learn-9775', 'scikit-learn__scikit-learn-9939', 'sphinx-doc__sphinx-11311', 'sphinx-doc__sphinx-7910', 'sympy__sympy-12812', 'sympy__sympy-14248', 'sympy__sympy-15222', 'sympy__sympy-19201'}
```

#####
            ruby

######
            FASTLANE_RSPEC_JQ_TRANSFORM

  `module-attribute`

```
FASTLANE_RSPEC_JQ_TRANSFORM = 'tail -n +2 | jq -r \'.examples[] | "\\(.description) - \\(.id) - \\(.status)"\''
```

######
            FPM_RSPEC_JQ_TRANSFORM

  `module-attribute`

```
FPM_RSPEC_JQ_TRANSFORM = 'sed -n \'/^{/,$p\' | jq -r \'.examples[] | "\\(.description) - \\(.status)"\''
```

######
            RUBOCOP_RSPEC_JQ_TRANSFORM

  `module-attribute`

```
RUBOCOP_RSPEC_JQ_TRANSFORM = strip()
```

######
            SPECS_JEKYLL

  `module-attribute`

```
SPECS_JEKYLL = {'9141': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['script/bootstrap'], 'test_cmd': ['bundle exec ruby -I test test/test_site.rb -v -n "/static files/"']}, '8761': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['script/bootstrap'], 'test_cmd': ['bundle exec cucumber --publish-quiet --format progress --no-color features/post_data.feature:6 features/post_data.feature:30']}, '8047': {'docker_specs': {'ruby_version': '3.3'}, 'pre_install': ["sed -i '/^[[:space:]]*install_if.*mingw/,/^[[:space:]]*end/d' Gemfile"], 'install': ['script/bootstrap', 'bundle add webrick'], 'test_cmd': ['bundle exec ruby -I test test/test_filters.rb -v -n "/where_exp filter/"']}, '8167': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['script/bootstrap', 'bundle add webrick'], 'test_cmd': ['bundle exec ruby -I test test/test_utils.rb -v -n "/Utils.slugify/"']}, '8771': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['script/bootstrap'], 'test_cmd': ['bundle exec cucumber --publish-quiet --format progress --no-color features/incremental_rebuild.feature:27 features/incremental_rebuild.feature:70']}}
```

######
            SPECS_FLUENTD

  `module-attribute`

```
SPECS_FLUENTD = {'4598': {'docker_specs': {'ruby_version': '3.3'}, 'pre_install': ['echo "gem \'console\', \'1.29\'" >> Gemfile'], 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin_helper/test_http_server_helper.rb -v -n '/mount/'"]}, '4311': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/config/test_system_config.rb -v -n '/rotate_age/'"]}, '4655': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_in_http.rb -v -n '/test_add/'"]}, '4030': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ['bundle exec ruby test/plugin/out_forward/test_ack_handler.rb -v']}, '3917': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ['bundle exec ruby test/test_config.rb -v']}, '3640': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin_helper/test_retry_state.rb -v -n '/exponential backoff/'"]}, '3641': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ['bundle exec ruby test/test_supervisor.rb -v']}, '3616': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_in_http.rb -v -n '/test_application/'"]}, '3631': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/test_event_router.rb -v -n '/handle_emits_error/'"]}, '3466': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_in_tail.rb -v -n '/test_should_replace_target_info/'"]}, '3328': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_in_tail.rb -v -n '/test_ENOENT_error_after_setup_watcher/'"]}, '3608': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/plugin/test_output_as_buffered_retries.rb -v -n '/retry_max_times/'"]}}
```

######
            SPECS_FASTLANE

  `module-attribute`

```
SPECS_FASTLANE = {'21857': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/lane_manager_base_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '20958': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/actions_specs/import_from_git_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '20642': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./frameit/spec/device_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '19765': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/actions_specs/download_dsyms_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '20975': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./match/spec/storage/s3_storage_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '19304': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/actions_specs/zip_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}, '19207': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install --jobs=$(nproc)'], 'test_cmd': [f'FASTLANE_SKIP_UPDATE_CHECK=1 bundle exec rspec ./fastlane/spec/actions_specs/zip_spec.rb --no-color --format json | {FASTLANE_RSPEC_JQ_TRANSFORM}']}}
```

######
            SPECS_FPM

  `module-attribute`

```
SPECS_FPM = {'1850': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/fpm/package/empty_spec.rb --no-color --format json | {FPM_RSPEC_JQ_TRANSFORM}']}, '1829': {'docker_specs': {'ruby_version': '3.1'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/fpm/package/deb_spec.rb --no-color --format json | {FPM_RSPEC_JQ_TRANSFORM}']}}
```

######
            SPECS_FAKER

  `module-attribute`

```
SPECS_FAKER = {'2970': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/faker/default/test_faker_internet.rb -v -n '/email/'"]}, '2705': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': ["bundle exec ruby test/faker/default/test_faker_internet.rb -v -n '/password/'"]}}
```

######
            SPECS_RUBOCOP

  `module-attribute`

```
SPECS_RUBOCOP = {'13705': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/lint/out_of_range_regexp_ref_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13687': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/lint/safe_navigation_chain_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13680': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/redundant_line_continuation_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13668': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/sole_nested_conditional_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13627': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/multiple_comparison_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13653': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/access_modifier_declarations_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13579': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/layout/line_continuation_spacing_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13560': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/file_null_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13503': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/dig_chain_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13479': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/layout/leading_comment_space_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13431': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/layout/empty_lines_around_method_body_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13424': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/safe_navigation_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13393': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/guard_clause_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13396': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/redundant_parentheses_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13375': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cli_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}, '13362': {'docker_specs': {'ruby_version': '3.3'}, 'install': ['bundle install'], 'test_cmd': [f'bundle exec rspec spec/rubocop/cop/style/redundant_freeze_spec.rb --no-color --format json | {RUBOCOP_RSPEC_JQ_TRANSFORM}']}}
```

######
            MAP_REPO_VERSION_TO_SPECS_RUBY

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_RUBY = {'jekyll/jekyll': SPECS_JEKYLL, 'fluent/fluentd': SPECS_FLUENTD, 'fastlane/fastlane': SPECS_FASTLANE, 'jordansissel/fpm': SPECS_FPM, 'faker-ruby/faker': SPECS_FAKER, 'rubocop/rubocop': SPECS_RUBOCOP}
```

######
            MAP_REPO_TO_INSTALL_RUBY

  `module-attribute`

```
MAP_REPO_TO_INSTALL_RUBY = {}
```

#####
            rust

######
            SPECS_RIPGREP

  `module-attribute`

```
SPECS_RIPGREP = {'2576': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ripgrep --test integration --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package ripgrep --test integration -- regression']}, '2209': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ripgrep --test integration --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package ripgrep --test integration -- regression::r2208 --exact']}}
```

######
            SPECS_BAT

  `module-attribute`

```
SPECS_BAT = {'3108': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests pag --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests pag']}, '2835': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests header --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests header']}, '2650': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests map_syntax --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests map_syntax']}, '2393': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests cache_ --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests cache_']}, '2201': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests pag --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests pag']}, '2260': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests syntax --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests syntax']}, '1892': {'docker_specs': {'rust_version': '1.81'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests ignored_suffix_arg --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests ignored_suffix_arg']}, '562': {'docker_specs': {'rust_version': '1.81'}, 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package bat --test integration_tests cache']}}
```

######
            SPECS_RUFF

  `module-attribute`

```
SPECS_RUFF = {'15626': {'docker_specs': {'rust_version': '1.84'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::flake8_simplify::tests --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::flake8_simplify::tests']}, '15543': {'docker_specs': {'rust_version': '1.84'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::pyupgrade --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::pyupgrade']}, '15443': {'docker_specs': {'rust_version': '1.84'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::flake8_bandit --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::flake8_bandit']}, '15394': {'docker_specs': {'rust_version': '1.83'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::flake8_pie --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::flake8_pie']}, '15356': {'docker_specs': {'rust_version': '1.83'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::pycodestyle --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::pycodestyle']}, '15330': {'docker_specs': {'rust_version': '1.83'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --lib rules::eradicate --no-run'], 'test_cmd': ['cargo test --package ruff_linter --lib rules::eradicate']}, '15309': {'docker_specs': {'rust_version': '1.83'}, 'install': ['RUSTFLAGS=-Awarnings cargo test --package ruff_linter --no-run'], 'test_cmd': ["cargo test --package ruff_linter 'f52'"]}}
```

######
            TOKIO_SPECS

  `module-attribute`

```
TOKIO_SPECS = {'6724': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6724.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test io_write_all_buf --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test io_write_all_buf --no-fail-fast']}, '6838': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6838.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test uds_stream --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test uds_stream --no-fail-fast']}, '6752': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6752.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test time_delay_queue --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test time_delay_queue --no-fail-fast']}, '4867': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-4867.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test sync_broadcast --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test sync_broadcast --no-fail-fast']}, '4898': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-4898.Cargo.lock'), 'install': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --locked --features full --test rt_metrics --no-run'], 'test_cmd': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --features full --test rt_metrics']}, '6603': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6603.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --test sync_mpsc --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --test sync_mpsc --no-fail-fast']}, '6551': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-6551.Cargo.lock'), 'install': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --locked --features full --test rt_metrics --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --features full --test rt_metrics --no-fail-fast']}, '4384': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-4384.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package tokio --test net_lookup_host --features full --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package tokio --test net_types_unwind --features full --no-fail-fast']}, '7139': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__tokio-7139.Cargo.lock'), 'install': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --locked --test fs_file --no-fail-fast --no-run'], 'test_cmd': ['RUSTFLAGS="-Awarnings --cfg tokio_unstable" cargo test --test fs_file --no-fail-fast']}}
```

######
            COREUTILS_SPECS

  `module-attribute`

```
COREUTILS_SPECS = {'6690': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test --no-run -- test_cp_cp test_cp_same_file test_cp_multiple_files test_cp_single_file test_cp_no_file'], 'test_cmd': ['cargo test --no-fail-fast -- test_cp_cp test_cp_same_file test_cp_multiple_files test_cp_single_file test_cp_no_file']}, '6731': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test backslash --no-run'], 'test_cmd': ['cargo test backslash --no-fail-fast']}, '6575': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test cksum --no-run'], 'test_cmd': ['cargo test cksum --no-fail-fast']}, '6682': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test mkdir --no-run'], 'test_cmd': ['cargo test mkdir --no-fail-fast']}, '6377': {'docker_specs': {'rust_version': '1.81'}, 'install': ['cargo test test_env --no-run'], 'test_cmd': ['cargo test test_env --no-fail-fast']}}
```

######
            NUSHELL_SPECS

  `module-attribute`

```
NUSHELL_SPECS = {'13246': {'docker_specs': {'rust_version': '1.77'}, 'install': ['cargo test -p nu-command --no-run --test main find::'], 'build': ['cargo build'], 'test_cmd': ['cargo test -p nu-command --no-fail-fast --test main find::']}, '12950': {'docker_specs': {'rust_version': '1.77'}, 'install': ['cargo test external_arguments --no-run'], 'test_cmd': ['cargo test external_arguments --no-fail-fast']}, '12901': {'docker_specs': {'rust_version': '1.77'}, 'install': ['cargo test --no-run shell::env'], 'test_cmd': ['cargo test --no-fail-fast shell::env']}, '13831': {'docker_specs': {'rust_version': '1.79'}, 'install': ['cargo test -p nu-command --no-run split_column'], 'build': ['cargo build'], 'test_cmd': ['cargo test -p nu-command --no-fail-fast split_column']}, '13605': {'docker_specs': {'rust_version': '1.78'}, 'install': ['cargo test -p nu-command --no-run ls::'], 'build': ['cargo build'], 'test_cmd': ['cargo test -p nu-command --no-fail-fast ls::']}}
```

######
            AXUM_SPECS

  `module-attribute`

```
AXUM_SPECS = {'2096': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-2096.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::fallback']}, '1934': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-1934.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::fallback']}, '1730': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-1730.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::mod state']}, '1119': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-1119.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib slash --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib slash']}, '734': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-734.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::head']}, '691': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-691.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib -- routing::tests::nest::nesting_router_at_root --exact']}, '682': {'docker_specs': {'rust_version': '1.83'}, 'pre_install': _write_cargo_lock_script('tokio-rs__axum-682.Cargo.lock'), 'install': ['RUSTFLAGS=-Awarnings cargo test --locked --package axum --lib trailing --no-run'], 'test_cmd': ['RUSTFLAGS=-Awarnings cargo test --package axum --lib trailing -- with_trailing_slash_post without_trailing_slash_post']}}
```

######
            MAP_REPO_VERSION_TO_SPECS_RUST

  `module-attribute`

```
MAP_REPO_VERSION_TO_SPECS_RUST = {'burntsushi/ripgrep': SPECS_RIPGREP, 'sharkdp/bat': SPECS_BAT, 'astral-sh/ruff': SPECS_RUFF, 'tokio-rs/tokio': TOKIO_SPECS, 'uutils/coreutils': COREUTILS_SPECS, 'nushell/nushell': NUSHELL_SPECS, 'tokio-rs/axum': AXUM_SPECS}
```

######
            MAP_REPO_TO_INSTALL_RUST

  `module-attribute`

```
MAP_REPO_TO_INSTALL_RUST = {}
```

####
            docker_build

#####
            BuildImageError

```
BuildImageError(image_name, message, logger)
```

              Bases: `Exception`

Source code in `swebench/harness/docker_build.py` | 28
29
30
31
32
33 | def __init__(self, image_name, message, logger):
    super().__init__(message)
    self.super_str = super().__str__()
    self.image_name = image_name
    self.log_path = logger.log_file
    self.logger = logger |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            super_str

  `instance-attribute`

```
super_str = __str__()
```

######
            image_name

  `instance-attribute`

```
image_name = image_name
```

######
            log_path

  `instance-attribute`

```
log_path = log_file
```

######
            logger

  `instance-attribute`

```
logger = logger
```

######
            __str__

```
__str__()
```

Source code in `swebench/harness/docker_build.py` | 35
36
37
38
39 | def __str__(self):
    return (
        f"Error building image {self.image_name}: {self.super_str}\n"
        f"Check ({self.log_path}) for more information."
    ) |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            setup_logger

```
setup_logger(instance_id: str, log_file: Path, mode='w', add_stdout: bool = False)
```

This logger is used for logging the build process of images and containers.
It writes logs to the log file.

If `add_stdout` is True, logs will also be sent to stdout, which can be used for
streaming ephemeral output from Modal containers.

Source code in `swebench/harness/docker_build.py` | 42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65
66 | def setup_logger(instance_id: str, log_file: Path, mode="w", add_stdout: bool = False):
    """
    This logger is used for logging the build process of images and containers.
    It writes logs to the log file.

    If `add_stdout` is True, logs will also be sent to stdout, which can be used for
    streaming ephemeral output from Modal containers.
    """
    log_file.parent.mkdir(parents=True, exist_ok=True)
    logger = logging.getLogger(f"{instance_id}.{log_file.name}")
    handler = logging.FileHandler(log_file, mode=mode, encoding=UTF8)
    formatter = logging.Formatter("%(asctime)s - %(levelname)s - %(message)s")
    handler.setFormatter(formatter)
    logger.addHandler(handler)
    logger.setLevel(logging.INFO)
    logger.propagate = False
    setattr(logger, "log_file", log_file)
    if add_stdout:
        handler = logging.StreamHandler(sys.stdout)
        formatter = logging.Formatter(
            f"%(asctime)s - {instance_id} - %(levelname)s - %(message)s"
        )
        handler.setFormatter(formatter)
        logger.addHandler(handler)
    return logger |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

#####
            close_logger

```
close_logger(logger)
```

Source code in `swebench/harness/docker_build.py` | 69
70
71
72
73 | def close_logger(logger):
    # To avoid too many open files
    for handler in logger.handlers:
        handler.close()
        logger.removeHandler(handler) |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            build_image

```
build_image(image_name: str, setup_scripts: dict, dockerfile: str, platform: str, client: DockerClient, build_dir: Path, nocache: bool = False)
```

Builds a docker image with the given name, setup scripts, dockerfile, and platform.

Parameters:

| Name          | Type         | Description                                                                      | Default  |
| ------------- | ------------ | -------------------------------------------------------------------------------- | -------- |
| image_name    | str          | Name of the image to build                                                       | required |
| setup_scripts | dict         | Dictionary of setup script names to setup script contents                        | required |
| dockerfile    | str          | Contents of the Dockerfile                                                       | required |
| platform      | str          | Platform to build the image for                                                  | required |
| client        | DockerClient | Docker client to use for building the image                                      | required |
| build_dir     | Path         | Directory for the build context (will also contain logs, scripts, and artifacts) | required |
| nocache       | bool         | Whether to use the cache when building                                           | False    |

Source code in `swebench/harness/docker_build.py` | 76
 77
 78
 79
 80
 81
 82
 83
 84
 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159 | def build_image(
    image_name: str,
    setup_scripts: dict,
    dockerfile: str,
    platform: str,
    client: docker.DockerClient,
    build_dir: Path,
    nocache: bool = False,
):
    """
    Builds a docker image with the given name, setup scripts, dockerfile, and platform.

    Args:
        image_name (str): Name of the image to build
        setup_scripts (dict): Dictionary of setup script names to setup script contents
        dockerfile (str): Contents of the Dockerfile
        platform (str): Platform to build the image for
        client (docker.DockerClient): Docker client to use for building the image
        build_dir (Path): Directory for the build context (will also contain logs, scripts, and artifacts)
        nocache (bool): Whether to use the cache when building
    """
    # Create a logger for the build process
    logger = setup_logger(image_name, build_dir / "build_image.log")
    logger.info(
        f"Building image {image_name}\n"
        f"Using dockerfile:\n{dockerfile}\n"
        f"Adding ({len(setup_scripts)}) setup scripts to image build repo"
    )

    for setup_script_name, setup_script in setup_scripts.items():
        logger.info(f"[SETUP SCRIPT] {setup_script_name}:\n{setup_script}")
    try:
        # Write the setup scripts to the build directory
        for setup_script_name, setup_script in setup_scripts.items():
            setup_script_path = build_dir / setup_script_name
            with open(setup_script_path, "w") as f:
                f.write(setup_script)
            if setup_script_name not in dockerfile:
                logger.warning(
                    f"Setup script {setup_script_name} may not be used in Dockerfile"
                )

        # Write the dockerfile to the build directory
        dockerfile_path = build_dir / "Dockerfile"
        with open(dockerfile_path, "w") as f:
            f.write(dockerfile)

        # Build the image
        logger.info(
            f"Building docker image {image_name} in {build_dir} with platform {platform}"
        )
        response = client.api.build(
            path=str(build_dir),
            tag=image_name,
            rm=True,
            forcerm=True,
            decode=True,
            platform=platform,
            nocache=nocache,
        )

        # Log the build process continuously
        buildlog = ""
        for chunk in response:
            if "stream" in chunk:
                # Remove ANSI escape sequences from the log
                chunk_stream = ansi_escape(chunk["stream"])
                logger.info(chunk_stream.strip())
                buildlog += chunk_stream
            elif "errorDetail" in chunk:
                # Decode error message, raise BuildError
                logger.error(f"Error: {ansi_escape(chunk['errorDetail']['message'])}")
                raise docker.errors.BuildError(
                    chunk["errorDetail"]["message"], buildlog
                )
        logger.info("Image built successfully!")
    except docker.errors.BuildError as e:
        logger.error(f"docker.errors.BuildError during {image_name}: {e}")
        raise BuildImageError(image_name, str(e), logger) from e
    except Exception as e:
        logger.error(f"Error building image {image_name}: {e}")
        raise BuildImageError(image_name, str(e), logger) from e
    finally:
        close_logger(logger)  # functions that create loggers should close them |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            build_base_images

```
build_base_images(client: DockerClient, dataset: list, force_rebuild: bool = False, namespace: str = None, instance_image_tag: str = None, env_image_tag: str = None)
```

Builds the base images required for the dataset if they do not already exist.

Parameters:

| Name          | Type         | Description                                                    | Default  |
| ------------- | ------------ | -------------------------------------------------------------- | -------- |
| client        | DockerClient | Docker client to use for building the images                   | required |
| dataset       | list         | List of test specs or dataset to build images for              | required |
| force_rebuild | bool         | Whether to force rebuild the images even if they already exist | False    |

Source code in `swebench/harness/docker_build.py` | 162
163
164
165
166
167
168
169
170
171
172
173
174
175
176
177
178
179
180
181
182
183
184
185
186
187
188
189
190
191
192
193
194
195
196
197
198
199
200
201
202
203
204
205
206
207
208
209
210
211
212 | def build_base_images(
    client: docker.DockerClient,
    dataset: list,
    force_rebuild: bool = False,
    namespace: str = None,
    instance_image_tag: str = None,
    env_image_tag: str = None,
):
    """
    Builds the base images required for the dataset if they do not already exist.

    Args:
        client (docker.DockerClient): Docker client to use for building the images
        dataset (list): List of test specs or dataset to build images for
        force_rebuild (bool): Whether to force rebuild the images even if they already exist
    """
    # Get the base images to build from the dataset
    test_specs = get_test_specs_from_dataset(
        dataset,
        namespace=namespace,
        instance_image_tag=instance_image_tag,
        env_image_tag=env_image_tag,
    )
    base_images = {
        x.base_image_key: (x.base_dockerfile, x.platform) for x in test_specs
    }

    # Build the base images
    for image_name, (dockerfile, platform) in base_images.items():
        try:
            # Check if the base image already exists
            client.images.get(image_name)
            if force_rebuild:
                # Remove the base image if it exists and force rebuild is enabled
                remove_image(client, image_name, "quiet")
            else:
                print(f"Base image {image_name} already exists, skipping build.")
                continue
        except docker.errors.ImageNotFound:
            pass
        # Build the base image (if it does not exist or force rebuild is enabled)
        print(f"Building base image ({image_name})")
        build_image(
            image_name=image_name,
            setup_scripts={},
            dockerfile=dockerfile,
            platform=platform,
            client=client,
            build_dir=BASE_IMAGE_BUILD_DIR / image_name.replace(":", "__"),
        )
    print("Base images built successfully.") |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_env_configs_to_build

```
get_env_configs_to_build(client: DockerClient, dataset: list, namespace: str = None, instance_image_tag: str = None, env_image_tag: str = None)
```

Returns a dictionary of image names to build scripts and dockerfiles for environment images.
Returns only the environment images that need to be built.

Parameters:

| Name    | Type         | Description                                       | Default  |
| ------- | ------------ | ------------------------------------------------- | -------- |
| client  | DockerClient | Docker client to use for building the images      | required |
| dataset | list         | List of test specs or dataset to build images for | required |

Source code in `swebench/harness/docker_build.py` | 215
216
217
218
219
220
221
222
223
224
225
226
227
228
229
230
231
232
233
234
235
236
237
238
239
240
241
242
243
244
245
246
247
248
249
250
251
252
253
254
255
256
257
258
259
260
261
262
263
264
265
266
267 | def get_env_configs_to_build(
    client: docker.DockerClient,
    dataset: list,
    namespace: str = None,
    instance_image_tag: str = None,
    env_image_tag: str = None,
):
    """
    Returns a dictionary of image names to build scripts and dockerfiles for environment images.
    Returns only the environment images that need to be built.

    Args:
        client (docker.DockerClient): Docker client to use for building the images
        dataset (list): List of test specs or dataset to build images for
    """
    image_scripts = dict()
    base_images = dict()
    test_specs = get_test_specs_from_dataset(
        dataset,
        namespace=namespace,
        instance_image_tag=instance_image_tag,
        env_image_tag=env_image_tag,
    )

    for test_spec in test_specs:
        # Check if the base image exists
        try:
            if test_spec.base_image_key not in base_images:
                base_images[test_spec.base_image_key] = client.images.get(
                    test_spec.base_image_key
                )
            base_image = base_images[test_spec.base_image_key]
        except docker.errors.ImageNotFound:
            raise Exception(
                f"Base image {test_spec.base_image_key} not found for {test_spec.env_image_key}\n."
                "Please build the base images first."
            )

        # Check if the environment image exists
        image_exists = False
        try:
            env_image = client.images.get(test_spec.env_image_key)
            image_exists = True
        except docker.errors.ImageNotFound:
            pass
        if not image_exists:
            # Add the environment image to the list of images to build
            image_scripts[test_spec.env_image_key] = {
                "setup_script": test_spec.setup_env_script,
                "dockerfile": test_spec.env_dockerfile,
                "platform": test_spec.platform,
            }
    return image_scripts |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            build_env_images

```
build_env_images(client: DockerClient, dataset: list, force_rebuild: bool = False, max_workers: int = 4, namespace: str = None, instance_image_tag: str = None, env_image_tag: str = None)
```

Builds the environment images required for the dataset if they do not already exist.

Parameters:

| Name          | Type         | Description                                                    | Default  |
| ------------- | ------------ | -------------------------------------------------------------- | -------- |
| client        | DockerClient | Docker client to use for building the images                   | required |
| dataset       | list         | List of test specs or dataset to build images for              | required |
| force_rebuild | bool         | Whether to force rebuild the images even if they already exist | False    |
| max_workers   | int          | Maximum number of workers to use for building images           | 4        |

Source code in `swebench/harness/docker_build.py` | 270
271
272
273
274
275
276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292
293
294
295
296
297
298
299
300
301
302
303
304
305
306
307
308
309
310
311
312
313
314
315
316
317
318
319
320
321
322
323
324
325
326
327
328
329
330
331
332
333 | def build_env_images(
    client: docker.DockerClient,
    dataset: list,
    force_rebuild: bool = False,
    max_workers: int = 4,
    namespace: str = None,
    instance_image_tag: str = None,
    env_image_tag: str = None,
):
    """
    Builds the environment images required for the dataset if they do not already exist.

    Args:
        client (docker.DockerClient): Docker client to use for building the images
        dataset (list): List of test specs or dataset to build images for
        force_rebuild (bool): Whether to force rebuild the images even if they already exist
        max_workers (int): Maximum number of workers to use for building images
    """
    # Get the environment images to build from the dataset
    if force_rebuild:
        env_image_keys = {
            x.env_image_key
            for x in get_test_specs_from_dataset(
                dataset,
                namespace=namespace,
                instance_image_tag=instance_image_tag,
                env_image_tag=env_image_tag,
            )
        }
        for key in env_image_keys:
            remove_image(client, key, "quiet")
    build_base_images(
        client, dataset, force_rebuild, namespace, instance_image_tag, env_image_tag
    )
    configs_to_build = get_env_configs_to_build(
        client, dataset, namespace, instance_image_tag, env_image_tag
    )
    if len(configs_to_build) == 0:
        print("No environment images need to be built.")
        return [], []
    print(f"Total environment images to build: {len(configs_to_build)}")

    args_list = list()
    for image_name, config in configs_to_build.items():
        args_list.append(
            (
                image_name,
                {"setup_env.sh": config["setup_script"]},
                config["dockerfile"],
                config["platform"],
                client,
                ENV_IMAGE_BUILD_DIR / image_name.replace(":", "__"),
            )
        )

    successful, failed = run_threadpool(build_image, args_list, max_workers)
    # Show how many images failed to build
    if len(failed) == 0:
        print("All environment images built successfully.")
    else:
        print(f"{len(failed)} environment images failed to build.")

    # Return the list of (un)successfuly built images
    return successful, failed |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            build_instance_images

```
build_instance_images(client: DockerClient, dataset: list, force_rebuild: bool = False, max_workers: int = 4, namespace: str = None, tag: str = None, env_image_tag: str = None)
```

Builds the instance images required for the dataset if they do not already exist.

Parameters:

| Name          | Type         | Description                                                    | Default  |
| ------------- | ------------ | -------------------------------------------------------------- | -------- |
| dataset       | list         | List of test specs or dataset to build images for              | required |
| client        | DockerClient | Docker client to use for building the images                   | required |
| force_rebuild | bool         | Whether to force rebuild the images even if they already exist | False    |
| max_workers   | int          | Maximum number of workers to use for building images           | 4        |

Source code in `swebench/harness/docker_build.py` | 336
337
338
339
340
341
342
343
344
345
346
347
348
349
350
351
352
353
354
355
356
357
358
359
360
361
362
363
364
365
366
367
368
369
370
371
372
373
374
375
376
377
378
379
380
381
382
383
384
385
386
387
388
389
390
391
392
393
394
395
396 | def build_instance_images(
    client: docker.DockerClient,
    dataset: list,
    force_rebuild: bool = False,
    max_workers: int = 4,
    namespace: str = None,
    tag: str = None,
    env_image_tag: str = None,
):
    """
    Builds the instance images required for the dataset if they do not already exist.

    Args:
        dataset (list): List of test specs or dataset to build images for
        client (docker.DockerClient): Docker client to use for building the images
        force_rebuild (bool): Whether to force rebuild the images even if they already exist
        max_workers (int): Maximum number of workers to use for building images
    """
    # Build environment images (and base images as needed) first
    test_specs = list(
        map(
            lambda x: make_test_spec(
                x,
                namespace=namespace,
                instance_image_tag=tag,
                env_image_tag=env_image_tag,
            ),
            dataset,
        )
    )
    if force_rebuild:
        for spec in test_specs:
            remove_image(client, spec.instance_image_key, "quiet")
    _, env_failed = build_env_images(client, test_specs, force_rebuild, max_workers)

    if len(env_failed) > 0:
        # Don't build images for instances that depend on failed-to-build env images
        dont_run_specs = [
            spec for spec in test_specs if spec.env_image_key in env_failed
        ]
        test_specs = [
            spec for spec in test_specs if spec.env_image_key not in env_failed
        ]
        print(
            f"Skipping {len(dont_run_specs)} instances - due to failed env image builds"
        )
    print(f"Building instance images for {len(test_specs)} instances")
    successful, failed = list(), list()

    # `logger` is set to None b/c logger is created in build-instage_image
    payloads = [(spec, client, None, False) for spec in test_specs]
    # Build the instance images
    successful, failed = run_threadpool(build_instance_image, payloads, max_workers)
    # Show how many images failed to build
    if len(failed) == 0:
        print("All instance images built successfully.")
    else:
        print(f"{len(failed)} instance images failed to build.")

    # Return the list of (un)successfuly built images
    return successful, failed |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            build_instance_image

```
build_instance_image(test_spec: TestSpec, client: DockerClient, logger: Logger | None, nocache: bool)
```

Builds the instance image for the given test spec if it does not already exist.

Parameters:

| Name      | Type         | Description                                 | Default  |
| --------- | ------------ | ------------------------------------------- | -------- |
| test_spec | TestSpec     | Test spec to build the instance image for   | required |
| client    | DockerClient | Docker client to use for building the image | required |
| logger    | Logger       | Logger to use for logging the build process | required |
| nocache   | bool         | Whether to use the cache when building      | required |

Source code in `swebench/harness/docker_build.py` | 399
400
401
402
403
404
405
406
407
408
409
410
411
412
413
414
415
416
417
418
419
420
421
422
423
424
425
426
427
428
429
430
431
432
433
434
435
436
437
438
439
440
441
442
443
444
445
446
447
448
449
450
451
452
453
454
455
456
457
458
459
460
461
462
463
464
465
466
467 | def build_instance_image(
    test_spec: TestSpec,
    client: docker.DockerClient,
    logger: logging.Logger \| None,
    nocache: bool,
):
    """
    Builds the instance image for the given test spec if it does not already exist.

    Args:
        test_spec (TestSpec): Test spec to build the instance image for
        client (docker.DockerClient): Docker client to use for building the image
        logger (logging.Logger): Logger to use for logging the build process
        nocache (bool): Whether to use the cache when building
    """
    # Set up logging for the build process
    build_dir = INSTANCE_IMAGE_BUILD_DIR / test_spec.instance_image_key.replace(
        ":", "__"
    )
    new_logger = False
    if logger is None:
        new_logger = True
        logger = setup_logger(test_spec.instance_id, build_dir / "prepare_image.log")

    # Get the image names and dockerfile for the instance image
    image_name = test_spec.instance_image_key
    env_image_name = test_spec.env_image_key
    dockerfile = test_spec.instance_dockerfile

    # Check that the env. image the instance image is based on exists
    try:
        env_image = client.images.get(env_image_name)
    except docker.errors.ImageNotFound as e:
        raise BuildImageError(
            test_spec.instance_id,
            f"Environment image {env_image_name} not found for {test_spec.instance_id}",
            logger,
        ) from e
    logger.info(
        f"Environment image {env_image_name} found for {test_spec.instance_id}\n"
        f"Building instance image {image_name} for {test_spec.instance_id}"
    )

    # Check if the instance image already exists
    image_exists = False
    try:
        client.images.get(image_name)
        image_exists = True
    except docker.errors.ImageNotFound:
        pass

    # Build the instance image
    if not image_exists:
        build_image(
            image_name=image_name,
            setup_scripts={
                "setup_repo.sh": test_spec.install_repo_script,
            },
            dockerfile=dockerfile,
            platform=test_spec.platform,
            client=client,
            build_dir=build_dir,
            nocache=nocache,
        )
    else:
        logger.info(f"Image {image_name} already exists, skipping build.")

    if new_logger:
        close_logger(logger) |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            build_container

```
build_container(test_spec: TestSpec, client: DockerClient, run_id: str, logger: Logger, nocache: bool, force_rebuild: bool = False)
```

Builds the instance image for the given test spec and creates a container from the image.

Parameters:

| Name          | Type         | Description                                                  | Default  |
| ------------- | ------------ | ------------------------------------------------------------ | -------- |
| test_spec     | TestSpec     | Test spec to build the instance image and container for      | required |
| client        | DockerClient | Docker client for building image + creating the container    | required |
| run_id        | str          | Run ID identifying process, used for the container name      | required |
| logger        | Logger       | Logger to use for logging the build process                  | required |
| nocache       | bool         | Whether to use the cache when building                       | required |
| force_rebuild | bool         | Whether to force rebuild the image even if it already exists | False    |

Source code in `swebench/harness/docker_build.py` | 470
471
472
473
474
475
476
477
478
479
480
481
482
483
484
485
486
487
488
489
490
491
492
493
494
495
496
497
498
499
500
501
502
503
504
505
506
507
508
509
510
511
512
513
514
515
516
517
518
519
520
521
522
523
524
525
526
527
528
529
530
531
532 | def build_container(
    test_spec: TestSpec,
    client: docker.DockerClient,
    run_id: str,
    logger: logging.Logger,
    nocache: bool,
    force_rebuild: bool = False,
):
    """
    Builds the instance image for the given test spec and creates a container from the image.

    Args:
        test_spec (TestSpec): Test spec to build the instance image and container for
        client (docker.DockerClient): Docker client for building image + creating the container
        run_id (str): Run ID identifying process, used for the container name
        logger (logging.Logger): Logger to use for logging the build process
        nocache (bool): Whether to use the cache when building
        force_rebuild (bool): Whether to force rebuild the image even if it already exists
    """
    # Build corresponding instance image
    if force_rebuild:
        remove_image(client, test_spec.instance_image_key, "quiet")
    if not test_spec.is_remote_image:
        build_instance_image(test_spec, client, logger, nocache)
    else:
        try:
            client.images.get(test_spec.instance_image_key)
        except docker.errors.ImageNotFound:
            try:
                client.images.pull(test_spec.instance_image_key)
            except docker.errors.NotFound as e:
                raise BuildImageError(test_spec.instance_id, str(e), logger) from e
            except Exception as e:
                raise Exception(
                    f"Error occurred while pulling image {test_spec.base_image_key}: {str(e)}"
                )

    container = None
    try:
        # Create the container
        logger.info(f"Creating container for {test_spec.instance_id}...")

        # Define arguments for running the container
        run_args = test_spec.docker_specs.get("run_args", {})
        cap_add = run_args.get("cap_add", [])

        container = client.containers.create(
            image=test_spec.instance_image_key,
            name=test_spec.get_instance_container_name(run_id),
            user=DOCKER_USER,
            detach=True,
            command="tail -f /dev/null",
            platform=test_spec.platform,
            cap_add=cap_add,
        )
        logger.info(f"Container for {test_spec.instance_id} created: {container.id}")
        return container
    except Exception as e:
        # If an error occurs, clean up the container and raise an exception
        logger.error(f"Error creating container for {test_spec.instance_id}: {e}")
        logger.info(traceback.format_exc())
        cleanup_container(client, container, logger)
        raise BuildImageError(test_spec.instance_id, str(e), logger) from e |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            docker_utils

#####
            HEREDOC_DELIMITER

  `module-attribute`

```
HEREDOC_DELIMITER = 'EOF_1399519320'
```

#####
            copy_to_container

```
copy_to_container(container: Container, src: Path, dst: Path)
```

Copy a file from local to a docker container

Parameters:

| Name      | Type      | Description                            | Default  |
| --------- | --------- | -------------------------------------- | -------- |
| container | Container | Docker container to copy to            | required |
| src       | Path      | Source file path                       | required |
| dst       | Path      | Destination file path in the container | required |

Source code in `swebench/harness/docker_utils.py` | 18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51 | def copy_to_container(container: Container, src: Path, dst: Path):
    """
    Copy a file from local to a docker container

    Args:
        container (Container): Docker container to copy to
        src (Path): Source file path
        dst (Path): Destination file path in the container
    """
    # Check if destination path is valid
    if os.path.dirname(dst) == "":
        raise ValueError(
            f"Destination path parent directory cannot be empty!, dst: {dst}"
        )

    # temporary tar file
    tar_path = src.with_suffix(".tar")
    with tarfile.open(tar_path, "w") as tar:
        tar.add(
            src, arcname=dst.name
        )  # use destination name, so after `put_archive`, name is correct

    # get bytes for put_archive cmd
    with open(tar_path, "rb") as tar_file:
        data = tar_file.read()

    # Make directory if necessary
    container.exec_run(f"mkdir -p {dst.parent}")

    # Send tar file to container and extract
    container.put_archive(os.path.dirname(dst), data)

    # clean up in locally and in container
    tar_path.unlink() |
| ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            write_to_container

```
write_to_container(container: Container, data: str, dst: Path)
```

Write a string to a file in a docker container

Source code in `swebench/harness/docker_utils.py` | 54
55
56
57
58
59
60 | def write_to_container(container: Container, data: str, dst: Path):
    """
    Write a string to a file in a docker container
    """
    # echo with heredoc to file
    command = f"cat <<'{HEREDOC_DELIMITER}' > {dst}\n{data}\n{HEREDOC_DELIMITER}"
    container.exec_run(command) |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            remove_image

```
remove_image(client, image_id, logger=None)
```

Remove a Docker image by ID.

Parameters:

| Name     | Type         | Description                                         | Default  |
| -------- | ------------ | --------------------------------------------------- | -------- |
| client   | DockerClient | Docker client.                                      | required |
| image_id | str          | Image ID.                                           | required |
| rm_image | bool         | Whether to remove the image.                        | required |
| logger   | Logger       | Logger to use for output. If None, print to stdout. | None     |

Source code in `swebench/harness/docker_utils.py` | 63
64
65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
80
81
82
83
84
85
86
87
88
89
90
91
92
93
94
95
96
97 | def remove_image(client, image_id, logger=None):
    """
    Remove a Docker image by ID.

    Args:
        client (docker.DockerClient): Docker client.
        image_id (str): Image ID.
        rm_image (bool): Whether to remove the image.
        logger (logging.Logger): Logger to use for output. If None, print to stdout.
    """
    if not logger:
        # if logger is None, print to stdout
        log_info = print
        log_error = print
        raise_error = True
    elif logger == "quiet":
        # if logger is "quiet", don't print anything
        log_info = lambda x: None
        log_error = lambda x: None
        raise_error = True
    else:
        # if logger is a logger object, use it
        log_error = logger.info
        log_info = logger.info
        raise_error = False
    try:
        log_info(f"Attempting to remove image {image_id}...")
        client.images.remove(image_id, force=True)
        log_info(f"Image {image_id} removed.")
    except docker.errors.ImageNotFound:
        log_info(f"Image {image_id} not found, removing has no effect.")
    except Exception as e:
        if raise_error:
            raise e
        log_error(f"Failed to remove image {image_id}: {e}\n{traceback.format_exc()}") |
| -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            cleanup_container

```
cleanup_container(client, container, logger)
```

Stop and remove a Docker container.
Performs this forcefully if the container cannot be stopped with the python API.

Parameters:

| Name      | Type         | Description                                        | Default  |
| --------- | ------------ | -------------------------------------------------- | -------- |
| client    | DockerClient | Docker client.                                     | required |
| container | Container    | Container to remove.                               | required |
| logger    | Logger       | Logger to use for output. If None, print to stdout | required |

Source code in `swebench/harness/docker_utils.py` | 100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159
160
161
162
163
164
165
166
167
168
169
170
171
172 | def cleanup_container(client, container, logger):
    """
    Stop and remove a Docker container.
    Performs this forcefully if the container cannot be stopped with the python API.

    Args:
        client (docker.DockerClient): Docker client.
        container (docker.models.containers.Container): Container to remove.
        logger (logging.Logger): Logger to use for output. If None, print to stdout
    """
    if not container:
        return

    container_id = container.id

    if not logger:
        # if logger is None, print to stdout
        log_error = print
        log_info = print
        raise_error = True
    elif logger == "quiet":
        # if logger is "quiet", don't print anything
        log_info = lambda x: None
        log_error = lambda x: None
        raise_error = True
    else:
        # if logger is a logger object, use it
        log_error = logger.info
        log_info = logger.info
        raise_error = False

    # Attempt to stop the container
    try:
        if container:
            log_info(f"Attempting to stop container {container.name}...")
            container.stop(timeout=15)
    except Exception as e:
        log_error(
            f"Failed to stop container {container.name}: {e}. Trying to forcefully kill..."
        )
        try:
            # Get the PID of the container
            container_info = client.api.inspect_container(container_id)
            pid = container_info["State"].get("Pid", 0)

            # If container PID found, forcefully kill the container
            if pid > 0:
                log_info(
                    f"Forcefully killing container {container.name} with PID {pid}..."
                )
                os.kill(pid, signal.SIGKILL)
            else:
                log_error(f"PID for container {container.name}: {pid} - not killing.")
        except Exception as e2:
            if raise_error:
                raise e2
            log_error(
                f"Failed to forcefully kill container {container.name}: {e2}\n"
                f"{traceback.format_exc()}"
            )

    # Attempt to remove the container
    try:
        log_info(f"Attempting to remove container {container.name}...")
        container.remove(force=True)
        log_info(f"Container {container.name} removed.")
    except Exception as e:
        if raise_error:
            raise e
        log_error(
            f"Failed to remove container {container.name}: {e}\n"
            f"{traceback.format_exc()}"
        ) |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            exec_run_with_timeout

```
exec_run_with_timeout(container, cmd, timeout: int | None = 60)
```

Run a command in a container with a timeout.

Parameters:

| Name      | Type      | Description                      | Default  |
| --------- | --------- | -------------------------------- | -------- |
| container | Container | Container to run the command in. | required |
| cmd       | str       | Command to run.                  | required |
| timeout   | int       | Timeout in seconds.              | 60       |

Source code in `swebench/harness/docker_utils.py` | 175
176
177
178
179
180
181
182
183
184
185
186
187
188
189
190
191
192
193
194
195
196
197
198
199
200
201
202
203
204
205
206
207
208
209
210
211
212
213
214
215
216
217 | def exec_run_with_timeout(container, cmd, timeout: int \| None = 60):
    """
    Run a command in a container with a timeout.

    Args:
        container (docker.Container): Container to run the command in.
        cmd (str): Command to run.
        timeout (int): Timeout in seconds.
    """
    # Local variables to store the result of executing the command
    exec_result = b""
    exec_id = None
    exception = None
    timed_out = False

    # Wrapper function to run the command
    def run_command():
        nonlocal exec_result, exec_id, exception
        try:
            exec_id = container.client.api.exec_create(container.id, cmd)["Id"]
            exec_stream = container.client.api.exec_start(exec_id, stream=True)
            for chunk in exec_stream:
                exec_result += chunk
        except Exception as e:
            exception = e

    # Start the command in a separate thread
    thread = threading.Thread(target=run_command)
    start_time = time.time()
    thread.start()
    thread.join(timeout)

    if exception:
        raise exception

    # If the thread is still alive, the command timed out
    if thread.is_alive():
        if exec_id is not None:
            exec_pid = container.client.api.exec_inspect(exec_id)["Pid"]
            container.exec_run(f"kill -TERM {exec_pid}", detach=True)
        timed_out = True
    end_time = time.time()
    return exec_result.decode(), timed_out, end_time - start_time |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            find_dependent_images

```
find_dependent_images(client: DockerClient, image_name: str)
```

Find all images that are built upon `image_name` image

Parameters:

| Name       | Type         | Description             | Default  |
| ---------- | ------------ | ----------------------- | -------- |
| client     | DockerClient | Docker client.          | required |
| image_name | str          | Name of the base image. | required |

Source code in `swebench/harness/docker_utils.py` | 220
221
222
223
224
225
226
227
228
229
230
231
232
233
234
235
236
237
238
239
240
241
242
243
244
245
246
247
248
249
250
251
252
253
254
255 | def find_dependent_images(client: docker.DockerClient, image_name: str):
    """
    Find all images that are built upon `image_name` image

    Args:
        client (docker.DockerClient): Docker client.
        image_name (str): Name of the base image.
    """
    dependent_images = []

    # Get all local images
    all_images = client.images.list()

    # Get the ID of the base image
    try:
        base_image = client.images.get(image_name)
        base_image_id = base_image.id
    except docker.errors.ImageNotFound:
        print(f"Base image {image_name} not found.")
        return []

    for image in all_images:
        # Skip the base image itself
        if image.id == base_image_id:
            continue

        # Check if the base image is in this image's history
        history = image.history()
        for layer in history:
            if layer["Id"] == base_image_id:
                # If found, add this image to the dependent images list
                tags = image.tags
                dependent_images.append(tags[0] if tags else image.id)
                break

    return dependent_images |
| ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            list_images

```
list_images(client: DockerClient)
```

List all images from the Docker client.

Source code in `swebench/harness/docker_utils.py` | 258
259
260
261
262
263 | def list_images(client: docker.DockerClient):
    """
    List all images from the Docker client.
    """
    # don't use this in multi-threaded context
    return {tag for i in client.images.list(all=True) for tag in i.tags} |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            clean_images

```
clean_images(client: DockerClient, prior_images: set, cache_level: str, clean: bool)
```

Clean Docker images based on cache level and clean flag.

Parameters:

| Name         | Type         | Description                                                                                                                                                                                                                                                                                                                   | Default  |
| ------------ | ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| client       | DockerClient | Docker client.                                                                                                                                                                                                                                                                                                                | required |
| prior_images | set          | Set of images that existed before the current run.                                                                                                                                                                                                                                                                            | required |
| cache        | str          | Cache level to use.                                                                                                                                                                                                                                                                                                           | required |
| clean        | bool         | Whether to clean; remove images that are higher in the cache hierarchy than the current
cache level. E.g. if cache_level is set to env, remove all previously built instances images. if
clean is false, previously built instances images will not be removed, but instance images built
in the current run will be removed. | required |

Source code in `swebench/harness/docker_utils.py` | 266
267
268
269
270
271
272
273
274
275
276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292 | def clean_images(
    client: docker.DockerClient, prior_images: set, cache_level: str, clean: bool
):
    """
    Clean Docker images based on cache level and clean flag.

    Args:
        client (docker.DockerClient): Docker client.
        prior_images (set): Set of images that existed before the current run.
        cache (str): Cache level to use.
        clean (bool): Whether to clean; remove images that are higher in the cache hierarchy than the current
            cache level. E.g. if cache_level is set to env, remove all previously built instances images. if
            clean is false, previously built instances images will not be removed, but instance images built
            in the current run will be removed.
    """
    images = list_images(client)
    removed = 0
    print("Cleaning cached images...")
    for image_name in images:
        if should_remove(image_name, cache_level, clean, prior_images):
            try:
                remove_image(client, image_name, "quiet")
                removed += 1
            except Exception as e:
                print(f"Error removing image {image_name}: {e}")
                continue
    print(f"Removed {removed} images.") |
| ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            should_remove

```
should_remove(image_name: str, cache_level: str, clean: bool, prior_images: set)
```

Determine if an image should be removed based on cache level and clean flag.

Source code in `swebench/harness/docker_utils.py` | 295
296
297
298
299
300
301
302
303
304
305
306
307
308
309
310
311 | def should_remove(image_name: str, cache_level: str, clean: bool, prior_images: set):
    """
    Determine if an image should be removed based on cache level and clean flag.
    """
    existed_before = image_name in prior_images
    if "/" in image_name:
        image_name = image_name.rsplit("/", 1)[-1]
    if image_name.startswith("sweb.base"):
        if cache_level in {"none"} and (clean or not existed_before):
            return True
    elif image_name.startswith("sweb.env"):
        if cache_level in {"none", "base"} and (clean or not existed_before):
            return True
    elif image_name.startswith("sweb.eval"):
        if cache_level in {"none", "base", "env"} and (clean or not existed_before):
            return True
    return False |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            dockerfiles

#####
            __all__

  `module-attribute`

```
__all__ = ['get_dockerfile_base', 'get_dockerfile_env', 'get_dockerfile_instance']
```

#####
            get_dockerfile_base

```
get_dockerfile_base(platform, arch, language, **kwargs)
```

Source code in `swebench/harness/dockerfiles/__init__.py` | 65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
80 | def get_dockerfile_base(platform, arch, language, **kwargs):
    if arch == "arm64":
        conda_arch = "aarch64"
    else:
        conda_arch = arch

    # Special handling for some js repos that require a different base image.
    # If other languages also start using variants, this logic should be moved
    # to a helper function
    if "_variant" in kwargs and kwargs["_variant"] == "js_2":
        del kwargs["_variant"]
        return _DOCKERFILE_BASE_JS_2.format(platform=platform, **kwargs)

    return _DOCKERFILE_BASE[language].format(
        platform=platform, conda_arch=conda_arch, **kwargs
    ) |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_dockerfile_env

```
get_dockerfile_env(platform, arch, language, base_image_key, **kwargs)
```

Source code in `swebench/harness/dockerfiles/__init__.py` | 83
84
85
86
87
88
89
90
91
92
93
94 | def get_dockerfile_env(platform, arch, language, base_image_key, **kwargs):
    # Some languages do not have an environment Dockerfile. In those cases, the
    # base Dockerfile is used as the environment Dockerfile.
    dockerfile = _DOCKERFILE_ENV.get(language, _DOCKERFILE_BASE[language])

    if "_variant" in kwargs and kwargs["_variant"] == "js_2":
        del kwargs["_variant"]
        return _DOCKERFILE_BASE_JS_2.format(platform=platform, **kwargs)

    return dockerfile.format(
        platform=platform, arch=arch, base_image_key=base_image_key, **kwargs
    ) |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_dockerfile_instance

```
get_dockerfile_instance(platform, language, env_image_name)
```

Source code in `swebench/harness/dockerfiles/__init__.py` | 97
 98
 99
100 | def get_dockerfile_instance(platform, language, env_image_name):
    return _DOCKERFILE_INSTANCE[language].format(
        platform=platform, env_image_name=env_image_name
    ) |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            c

#####
            go

#####
            java

#####
            javascript

#####
            php

#####
            python

#####
            ruby

#####
            rust

####
            grading

#####
            test_passed

```
test_passed(case: str, sm: dict[str, str]) -> bool
```

Source code in `swebench/harness/grading.py` | 27
28 | def test_passed(case: str, sm: dict[str, str]) -> bool:
    return case in sm and sm[case] in [TestStatus.PASSED.value, TestStatus.XFAIL.value] |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            test_failed

```
test_failed(case: str, sm: dict[str, str]) -> bool
```

Source code in `swebench/harness/grading.py` | 31
32
33
34
35 | def test_failed(case: str, sm: dict[str, str]) -> bool:
    return case not in sm or sm[case] in [
        TestStatus.FAILED.value,
        TestStatus.ERROR.value,
    ] |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_logs_eval

```
get_logs_eval(test_spec: TestSpec, log_fp: str) -> tuple[dict[str, str], bool]
```

Retrieve evaluation results for a task instance from its corresponding log file

Parameters:

| Name   | Type | Description      | Default  |
| ------ | ---- | ---------------- | -------- |
| log_fp | str  | path to log file | required |

Returns:
    bool: whether the patch applied successfully
    dict: status map

TODO(john-b-yang): Check this is working properly...

Source code in `swebench/harness/grading.py` | 39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
80
81
82
83
84
85
86
87
88
89
90
91 | def get_logs_eval(test_spec: TestSpec, log_fp: str) -> tuple[dict[str, str], bool]:
    """
    Retrieve evaluation results for a task instance from its corresponding log file

    Args:
        log_fp (str): path to log file
    Returns:
        bool: whether the patch applied successfully
        dict: status map

    TODO(john-b-yang): Check this is working properly...
    """
    repo = test_spec.repo
    version = test_spec.version
    log_parser = MAP_REPO_TO_PARSER[repo]
    test_cmd = MAP_REPO_VERSION_TO_SPECS[repo][version]["test_cmd"]
    if isinstance(test_cmd, list):
        test_cmd = test_cmd[-1]

    with open(log_fp) as f:
        content = f.read()
        # TODO fix constant here
        bad_codes = list(
            filter(
                lambda x: x in content,
                [
                    APPLY_PATCH_FAIL,
                    RESET_FAILED,
                    TESTS_ERROR,
                    TESTS_TIMEOUT,
                ],
            )
        )
        if bad_codes:
            return {}, False
        elif not (START_TEST_OUTPUT in content and END_TEST_OUTPUT in content):
            # Test patch did not apply (should not happen at all)
            return {}, False

        # Get status map of evaluation results
        test_content = content.split(START_TEST_OUTPUT)[1].split(END_TEST_OUTPUT)[0]

        # Try parsing the content between markers first
        status_map = log_parser(test_content, test_spec)

        # If no test results found between markers (common in Modal environment),
        # try parsing the entire log content as fallback
        if not status_map:
            # Look for pytest output patterns in the entire log content
            # This handles cases where pytest output goes to stderr and isn't captured between markers
            status_map = log_parser(content, test_spec)

        return status_map, True |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_eval_tests_report

```
get_eval_tests_report(eval_status_map: dict[str, str], gold_results: dict[str, str], calculate_to_fail: bool = False, eval_type: EvalType = PASS_AND_FAIL) -> dict[str, dict[str, list[str]]]
```

Create a report based on failure/pass change from gold results to eval results.

Parameters:

| Name              | Type | Description                                        | Default  |
| ----------------- | ---- | -------------------------------------------------- | -------- |
| eval_sm           | dict | evaluation status map                              | required |
| gold_results      | dict | gold results                                       | required |
| calculate_to_fail | bool | whether to calculate metrics for "x to fail" tests | False    |

Returns:
    report (dict): report of metrics

Metric Definitions (Gold Result Pair + Eval Result):
- Fail-Pass (F2P) + P: Success (Resolution)
- Pass-Pass (P2P) + P: Success (Maintenance)
- Fail-Pass (F2P) + F: Failure
- Pass-Pass (P2P) + F: Failure

Miscellaneous Definitions
- Fail-Fail (F2F) + F: Failure Maintenance
- Pass-Fail (P2F) + F: Not considered
- Fail-Fail (F2F) + P: Success (Extra Credit)
- Pass-Fail (P2F) + P: Not considered

Source code in `swebench/harness/grading.py` | 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159
160
161
162
163
164
165
166
167
168
169
170
171
172
173
174
175
176
177
178
179
180
181
182
183
184
185
186
187
188
189
190
191 | def get_eval_tests_report(
    eval_status_map: dict[str, str],
    gold_results: dict[str, str],
    calculate_to_fail: bool = False,
    eval_type: EvalType = EvalType.PASS_AND_FAIL,
) -> dict[str, dict[str, list[str]]]:
    """
    Create a report based on failure/pass change from gold results to eval results.

    Args:
        eval_sm (dict): evaluation status map
        gold_results (dict): gold results
        calculate_to_fail (bool): whether to calculate metrics for "x to fail" tests
    Returns:
        report (dict): report of metrics

    Metric Definitions (Gold Result Pair + Eval Result):
    - Fail-Pass (F2P) + P: Success (Resolution)
    - Pass-Pass (P2P) + P: Success (Maintenance)
    - Fail-Pass (F2P) + F: Failure
    - Pass-Pass (P2P) + F: Failure

    Miscellaneous Definitions
    - Fail-Fail (F2F) + F: Failure Maintenance
    - Pass-Fail (P2F) + F: Not considered
    - Fail-Fail (F2F) + P: Success (Extra Credit)
    - Pass-Fail (P2F) + P: Not considered
    """

    def check_pass_and_fail(test_case, eval_status_map, success, failed):
        if test_passed(test_case, eval_status_map):
            # Assume silent success for now (test case not in eval_sm)
            success.append(test_case)
        elif test_failed(test_case, eval_status_map):
            failed.append(test_case)

    def check_fail_only(test_case, eval_status_map, success, failed):
        if (
            test_case in eval_status_map
            and eval_status_map[test_case] == TestStatus.FAILED.value
        ):
            failed.append(test_case)
        else:
            success.append(test_case)

    check_test_case = (
        check_pass_and_fail if eval_type == EvalType.PASS_AND_FAIL else check_fail_only
    )

    # Calculate resolution metrics
    f2p_success = []
    f2p_failure = []
    for test_case in gold_results[FAIL_TO_PASS]:
        check_test_case(test_case, eval_status_map, f2p_success, f2p_failure)

    # Calculate maintenance metrics
    p2p_success = []
    p2p_failure = []
    for test_case in gold_results[PASS_TO_PASS]:
        check_test_case(test_case, eval_status_map, p2p_success, p2p_failure)

    results = {
        FAIL_TO_PASS: {
            "success": f2p_success,
            "failure": f2p_failure,
        },
        PASS_TO_PASS: {
            "success": p2p_success,
            "failure": p2p_failure,
        },
    }

    f2f_success = []
    f2f_failure = []
    p2f_success = []
    p2f_failure = []
    if calculate_to_fail:
        # Calculate "extra credit" metrics
        for test_case in gold_results[FAIL_TO_FAIL]:
            check_test_case(test_case, eval_status_map, f2f_success, f2f_failure)

        # Calculate not considered metrics
        for test_case in gold_results[PASS_TO_FAIL]:
            check_test_case(test_case, eval_status_map, p2f_success, p2f_failure)

    results.update(
        {
            FAIL_TO_FAIL: {
                "success": f2f_success,
                "failure": f2f_failure,
            },
            PASS_TO_FAIL: {
                "success": p2f_success,
                "failure": p2f_failure,
            },
        }
    )
    return results |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            compute_fail_to_pass

```
compute_fail_to_pass(report: dict[str, dict[str, Any]]) -> float
```

Compute fail-to-pass metric. Accepts single report as argument.

Source code in `swebench/harness/grading.py` | 194
195
196
197
198
199
200
201 | def compute_fail_to_pass(report: dict[str, dict[str, Any]]) -> float:
    """
    Compute fail-to-pass metric. Accepts single report as argument.
    """
    total = len(report[FAIL_TO_PASS]["success"]) + len(report[FAIL_TO_PASS]["failure"])
    if total == 0:
        return 1
    return len(report[FAIL_TO_PASS]["success"]) / total |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            compute_pass_to_pass

```
compute_pass_to_pass(report: dict[str, dict[str, Any]]) -> float
```

Compute pass-to-pass metric. Accepts single report as argument.

Source code in `swebench/harness/grading.py` | 204
205
206
207
208
209
210
211
212 | def compute_pass_to_pass(report: dict[str, dict[str, Any]]) -> float:
    """
    Compute pass-to-pass metric. Accepts single report as argument.
    """
    total = len(report[PASS_TO_PASS]["success"]) + len(report[PASS_TO_PASS]["failure"])
    if total == 0:
        # TODO: Don't factor in p2p metrics
        return 1
    return len(report[PASS_TO_PASS]["success"]) / total |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_resolution_status

```
get_resolution_status(report: dict[str, dict[str, Any]]) -> str
```

Determine resolved status of an evaluation instance

Criteria - If fail-to-pass (Resolution) = 1 and pass-to-pass (Maintenance) = 1 -> FULL
- If (fail-to-pass (Resolution) < 1 and > 0) and pass-to-pass (Maintenance) = 1 -> PARTIAL
- Otherwise -> NO

Source code in `swebench/harness/grading.py` | 215
216
217
218
219
220
221
222
223
224
225
226
227
228
229
230
231
232 | def get_resolution_status(report: dict[str, dict[str, Any]]) -> str:
    """
    Determine resolved status of an evaluation instance

    Criteria:
        - If fail-to-pass (Resolution) = 1 and pass-to-pass (Maintenance) = 1 -> FULL
        - If (fail-to-pass (Resolution) < 1 and > 0) and pass-to-pass (Maintenance) = 1 -> PARTIAL
        - Otherwise -> NO
    """
    f2p = compute_fail_to_pass(report)
    p2p = compute_pass_to_pass(report)

    if f2p == 1 and p2p == 1:
        return ResolvedStatus.FULL.value
    elif f2p < 1 and f2p > 0 and p2p == 1:
        return ResolvedStatus.PARTIAL.value
    else:
        return ResolvedStatus.NO.value |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_eval_report

```
get_eval_report(test_spec: TestSpec, prediction: dict[str, str], test_log_path: str, include_tests_status: bool) -> dict[str, Any]
```

Generate a report of model evaluation results from a prediction, task instance,
and evaluation log.

Parameters:

| Name                 | Type | Description                                                                       | Default  |
| -------------------- | ---- | --------------------------------------------------------------------------------- | -------- |
| test_spec            | dict | test spec containing keys "instance_id", "FAIL_TO_PASS", and "PASS_TO_PASS"       | required |
| prediction           | dict | prediction containing keys "instance_id", "model_name_or_path", and "model_patch" | required |
| log_path             | str  | path to evaluation log                                                            | required |
| include_tests_status | bool | whether to include the status of each test in the returned report                 | required |

Returns:
    report (dict): report of metrics

Source code in `swebench/harness/grading.py` | 235
236
237
238
239
240
241
242
243
244
245
246
247
248
249
250
251
252
253
254
255
256
257
258
259
260
261
262
263
264
265
266
267
268
269
270
271
272
273
274
275
276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292
293
294
295 | def get_eval_report(
    test_spec: TestSpec,
    prediction: dict[str, str],
    test_log_path: str,
    include_tests_status: bool,
) -> dict[str, Any]:
    """
    Generate a report of model evaluation results from a prediction, task instance,
    and evaluation log.

    Args:
        test_spec (dict): test spec containing keys "instance_id", "FAIL_TO_PASS", and "PASS_TO_PASS"
        prediction (dict): prediction containing keys "instance_id", "model_name_or_path", and "model_patch"
        log_path (str): path to evaluation log
        include_tests_status (bool): whether to include the status of each test in the returned report
    Returns:
        report (dict): report of metrics
    """
    report_map = {}

    instance_id = prediction[KEY_INSTANCE_ID]
    report_map[instance_id] = {
        "patch_is_None": False,
        "patch_exists": False,
        "patch_successfully_applied": False,
        "resolved": False,
    }

    # Check if the model patch exists
    if prediction[KEY_PREDICTION] is None:
        report_map[instance_id]["patch_is_None"] = True
        return report_map
    report_map[instance_id]["patch_exists"] = True

    # Get evaluation logs
    eval_status_map, found = get_logs_eval(test_spec, test_log_path)

    if not found:
        return report_map
    report_map[instance_id]["patch_successfully_applied"] = True

    eval_ref = {
        KEY_INSTANCE_ID: test_spec.instance_id,
        FAIL_TO_PASS: test_spec.FAIL_TO_PASS,
        PASS_TO_PASS: test_spec.PASS_TO_PASS,
    }

    eval_type = (
        EvalType.FAIL_ONLY
        if test_spec.repo in FAIL_ONLY_REPOS
        else EvalType.PASS_AND_FAIL
    )

    report = get_eval_tests_report(eval_status_map, eval_ref, eval_type=eval_type)
    if get_resolution_status(report) == ResolvedStatus.FULL.value:
        report_map[instance_id]["resolved"] = True

    if include_tests_status:
        report_map[instance_id]["tests_status"] = report  # type: ignore

    return report_map |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            log_parsers

#####
            MAP_REPO_TO_PARSER

  `module-attribute`

```
MAP_REPO_TO_PARSER = {None: MAP_REPO_TO_PARSER_C, None: MAP_REPO_TO_PARSER_GO, None: MAP_REPO_TO_PARSER_JAVA, None: MAP_REPO_TO_PARSER_JS, None: MAP_REPO_TO_PARSER_PHP, None: MAP_REPO_TO_PARSER_PY, None: MAP_REPO_TO_PARSER_RUST, None: MAP_REPO_TO_PARSER_RUBY}
```

#####
            __all__

  `module-attribute`

```
__all__ = ['MAP_REPO_TO_PARSER']
```

#####
            c

######
            MAP_REPO_TO_PARSER_C

  `module-attribute`

```
MAP_REPO_TO_PARSER_C = {'redis/redis': parse_log_redis, 'jqlang/jq': parse_log_jq, 'nlohmann/json': parse_log_doctest, 'micropython/micropython': parse_log_micropython_test, 'valkey-io/valkey': parse_log_redis, 'fmtlib/fmt': parse_log_googletest}
```

######
            parse_log_redis

```
parse_log_redis(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/c.py` | 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32 | def parse_log_redis(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}

    pattern = r"^\[(ok\|err\|skip\|ignore)\]:\s(.+?)(?:\s\((\d+\s*m?s)\))?$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status, test_name, _duration = match.groups()
            if status == "ok":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status == "err":
                # Strip out file path information from failed test names
                test_name = re.sub(r"\s+in\s+\S+$", "", test_name)
                test_status_map[test_name] = TestStatus.FAILED.value
            elif status == "skip" or status == "ignore":
                test_status_map[test_name] = TestStatus.SKIPPED.value

    return test_status_map |
| ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_jq

```
parse_log_jq(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/c.py` | 35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54 | def parse_log_jq(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}

    pattern = r"^\s*(PASS\|FAIL):\s(.+)$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status, test_name = match.groups()
            if status == "PASS":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status == "FAIL":
                test_status_map[test_name] = TestStatus.FAILED.value
    return test_status_map |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_doctest

```
parse_log_doctest(log: str, test_spec: TestSpec) -> dict[str, str]
```

Assumes test binary runs with -s -r=xml.

Source code in `swebench/harness/log_parsers/c.py` | 57
58
59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
80
81
82
83
84
85
86
87
88
89
90
91 | def parse_log_doctest(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Assumes test binary runs with -s -r=xml.
    """
    test_status_map = {}

    # Extract XML content
    start_tag = "<doctest"
    end_tag = "</doctest>"
    start_index = log.find(start_tag)
    end_index = (
        log.find(end_tag, start_index) + len(end_tag) if start_index != -1 else -1
    )

    if start_index != -1 and end_index != -1:
        xml_string = log[start_index:end_index]
        root = ET.fromstring(xml_string)

        for testcase in root.findall(".//TestCase"):
            testcase_name = testcase.get("name")
            for subcase in testcase.findall(".//SubCase"):
                subcase_name = subcase.get("name")
                name = f"{testcase_name} > {subcase_name}"

                expressions = subcase.findall(".//Expression")
                subcase_passed = all(
                    expr.get("success") == "true" for expr in expressions
                )

                if subcase_passed:
                    test_status_map[name] = TestStatus.PASSED.value
                else:
                    test_status_map[name] = TestStatus.FAILED.value

    return test_status_map |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_micropython_test

```
parse_log_micropython_test(log: str, test_spec: TestSpec) -> dict[str, str]
```

Source code in `swebench/harness/log_parsers/c.py` | 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110 | def parse_log_micropython_test(log: str, test_spec: TestSpec) -> dict[str, str]:
    test_status_map = {}

    pattern = r"^(pass\|FAIL\|skip)\s+(.+)$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status, test_name = match.groups()
            if status == "pass":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status == "FAIL":
                test_status_map[test_name] = TestStatus.FAILED.value
            elif status == "skip":
                test_status_map[test_name] = TestStatus.SKIPPED.value

    return test_status_map |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_googletest

```
parse_log_googletest(log: str, test_spec: TestSpec) -> dict[str, str]
```

Source code in `swebench/harness/log_parsers/c.py` | 113
114
115
116
117
118
119
120
121
122
123
124
125
126
127 | def parse_log_googletest(log: str, test_spec: TestSpec) -> dict[str, str]:
    test_status_map = {}

    pattern = r"^.*\[\s*(OK\|FAILED)\s*\]\s(.*)\s\(.*\)$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status, test_name = match.groups()
            if status == "OK":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status == "FAILED":
                test_status_map[test_name] = TestStatus.FAILED.value

    return test_status_map |
| ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            go

######
            MAP_REPO_TO_PARSER_GO

  `module-attribute`

```
MAP_REPO_TO_PARSER_GO = {'caddyserver/caddy': parse_log_gotest, 'hashicorp/terraform': parse_log_gotest, 'prometheus/prometheus': parse_log_gotest, 'gohugoio/hugo': parse_log_gotest, 'gin-gonic/gin': parse_log_gotest}
```

######
            parse_log_gotest

```
parse_log_gotest(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with 'go test'

Parameters:

| Name      | Type     | Description        | Default  |
| --------- | -------- | ------------------ | -------- |
| log       | str      | log content        | required |
| test_spec | TestSpec | test spec (unused) | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/go.py` | 6
 7
 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32 | def parse_log_gotest(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with 'go test'

    Args:
        log (str): log content
        test_spec (TestSpec): test spec (unused)
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}

    # Pattern to match test result lines
    pattern = r"^--- (PASS\|FAIL\|SKIP): (.+) \((.+)\)$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status, test_name, _duration = match.groups()
            if status == "PASS":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status == "FAIL":
                test_status_map[test_name] = TestStatus.FAILED.value
            elif status == "SKIP":
                test_status_map[test_name] = TestStatus.SKIPPED.value

    return test_status_map |
| ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            java

######
            MAP_REPO_TO_PARSER_JAVA

  `module-attribute`

```
MAP_REPO_TO_PARSER_JAVA = {'google/gson': parse_log_maven, 'apache/druid': parse_log_maven, 'javaparser/javaparser': parse_log_maven, 'projectlombok/lombok': parse_log_ant, 'apache/lucene': parse_log_gradle_custom, 'reactivex/rxjava': parse_log_gradle_custom}
```

######
            parse_log_maven

```
parse_log_maven(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with 'mvn test'.
Annoyingly maven will not print the tests that have succeeded. For this log
parser to work, each test must be run individually, and then we look for
BUILD (SUCCESS|FAILURE) in the logs.

Handles race conditions where multiple test commands appear before their
BUILD results due to concurrent output from shell tracing and Maven.

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/java.py` | 6
 7
 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65 | def parse_log_maven(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with 'mvn test'.
    Annoyingly maven will not print the tests that have succeeded. For this log
    parser to work, each test must be run individually, and then we look for
    BUILD (SUCCESS\|FAILURE) in the logs.

    Handles race conditions where multiple test commands appear before their
    BUILD results due to concurrent output from shell tracing and Maven.

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}
    pending_tests: list[str] = []
    unmatched_results: list[str] = []

    # Get the test name from the command used to execute the test.
    # Assumes we run evaluation with set -x
    test_name_pattern = r"^.*-Dtest=(\S+).*$"
    result_pattern = r"^.*BUILD (SUCCESS\|FAILURE)$"

    for line in log.split("\n"):
        test_name_match = re.match(test_name_pattern, line.strip())
        if test_name_match:
            pending_tests.append(test_name_match.groups()[0])

        result_match = re.match(result_pattern, line.strip())
        if result_match:
            status = result_match.groups()[0]
            if pending_tests:
                test_name = pending_tests.pop(0)
                if status == "SUCCESS":
                    test_status_map[test_name] = TestStatus.PASSED.value
                elif status == "FAILURE":
                    test_status_map[test_name] = TestStatus.FAILED.value
            else:
                # Track unmatched results for later matching
                unmatched_results.append(status)

    # Match any remaining pending tests with unmatched results (FIFO order)
    # This handles cases where BUILD results appear after other output
    while pending_tests and unmatched_results:
        test_name = pending_tests.pop(0)
        status = unmatched_results.pop(0)
        if status == "SUCCESS":
            test_status_map[test_name] = TestStatus.PASSED.value
        elif status == "FAILURE":
            test_status_map[test_name] = TestStatus.FAILED.value

    # Warn if there are still pending tests without results
    if pending_tests:
        print(
            f"[WARNING] Maven log parser: {len(pending_tests)} test(s) had no BUILD result: "
            f"{pending_tests}"
        )

    return test_status_map |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_ant

```
parse_log_ant(log: str, test_spec: TestSpec) -> dict[str, str]
```

Source code in `swebench/harness/log_parsers/java.py` | 68
69
70
71
72
73
74
75
76
77
78
79
80
81
82 | def parse_log_ant(log: str, test_spec: TestSpec) -> dict[str, str]:
    test_status_map = {}

    pattern = r"^\s*\[junit\]\s+\[(PASS\|FAIL\|ERR)\]\s+(.*)$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status, test_name = match.groups()
            if status == "PASS":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status in ["FAIL", "ERR"]:
                test_status_map[test_name] = TestStatus.FAILED.value

    return test_status_map |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            parse_log_gradle_custom

```
parse_log_gradle_custom(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with 'gradle test'. Assumes that the
pre-install script to update the gradle config has run.

Handles race conditions where test name and status appear on different lines
due to interleaved log output from concurrent processes.

Source code in `swebench/harness/log_parsers/java.py` | 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147 | def parse_log_gradle_custom(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with 'gradle test'. Assumes that the
    pre-install script to update the gradle config has run.

    Handles race conditions where test name and status appear on different lines
    due to interleaved log output from concurrent processes.
    """
    test_status_map = {}

    # Pattern for normal case: test name and status on the same line
    # e.g., "com.example.Test > testMethod PASSED"
    # [^>] ensures we don't match lines starting with > (shell prompts, etc.)
    full_pattern = r"^([^>].+)\s+(PASSED\|FAILED)$"

    # Pattern for test name without status (race condition case)
    # e.g., "com.example.Test > testMethod" followed by warnings, then "PASSED"
    # Must also start with [^>] for consistency
    test_name_pattern = r"^([^>]\S*\s+>\s+\S+)$"

    # Pattern for standalone status line
    status_only_pattern = r"^(PASSED\|FAILED)$"

    pending_test_name = None

    for line in log.split("\n"):
        stripped = line.strip()

        # Check for full match (test name + status on same line)
        match = re.match(full_pattern, stripped)
        if match:
            test_name, status = match.groups()
            if status == "PASSED":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status == "FAILED":
                test_status_map[test_name] = TestStatus.FAILED.value
            pending_test_name = None
            continue

        # Check for test name without status
        test_name_match = re.match(test_name_pattern, stripped)
        if test_name_match:
            pending_test_name = test_name_match.group(1)
            continue

        # Check for standalone status (applies to pending test name)
        if pending_test_name:
            status_match = re.match(status_only_pattern, stripped)
            if status_match:
                status = status_match.group(1)
                if status == "PASSED":
                    test_status_map[pending_test_name] = TestStatus.PASSED.value
                elif status == "FAILED":
                    test_status_map[pending_test_name] = TestStatus.FAILED.value
                pending_test_name = None

    # Warn if there's a pending test without a result
    if pending_test_name:
        print(
            f"[WARNING] Gradle log parser: test had no status result: {pending_test_name}"
        )

    return test_status_map |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            javascript

######
            MAP_REPO_TO_PARSER_JS

  `module-attribute`

```
MAP_REPO_TO_PARSER_JS = {'Automattic/wp-calypso': parse_log_calypso, 'chartjs/Chart.js': parse_log_chart_js, 'markedjs/marked': parse_log_marked, 'processing/p5.js': parse_log_p5js, 'diegomura/react-pdf': parse_log_react_pdf, 'babel/babel': parse_log_jest, 'vuejs/core': parse_log_vitest, 'facebook/docusaurus': parse_log_jest, 'immutable-js/immutable-js': parse_log_immutable_js, 'mrdoob/three.js': parse_log_tap, 'preactjs/preact': parse_log_karma, 'axios/axios': parse_log_tap}
```

######
            parse_log_calypso

```
parse_log_calypso(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated by Calypso test suite

Source code in `swebench/harness/log_parsers/javascript.py` | 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55 | def parse_log_calypso(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated by Calypso test suite
    """
    test_status_map = {}
    suite = []

    get_test_name = lambda suite, match_pattern, line: " - ".join(
        [" - ".join([x[0] for x in suite]), re.match(match_pattern, line).group(1)]
    ).strip()

    for log in log.split(" ./node_modules/.bin/jest ")[1:]:
        for line in log.split("\n"):
            if any([line.startswith(x) for x in ["Test Suites", "  ● "]]):
                break
            elif line.strip().startswith("✓"):
                # Test passed
                match_pattern = (
                    r"^\s+✓\s(.*)\(\d+ms\)$"
                    if re.search(r"\(\d+ms\)", line) is not None
                    else r"^\s+✓\s(.*)"
                )
                test_status_map[get_test_name(suite, match_pattern, line)] = (
                    TestStatus.PASSED.value
                )
            elif line.strip().startswith("✕"):
                # Test failed
                match_pattern = (
                    r"^\s+✕\s(.*)\(\d+ms\)$"
                    if re.search(r"\(\d+ms\)", line) is not None
                    else r"^\s+✕\s(.*)"
                )
                test_status_map[get_test_name(suite, match_pattern, line)] = (
                    TestStatus.FAILED.value
                )
            elif len(line) - len(line.lstrip()) > 0:
                # Adjust suite name
                indent = len(line) - len(line.lstrip())
                if len(suite) == 0:
                    # If suite is empty, initialize it
                    suite = [(line.strip(), indent)]
                else:
                    while len(suite) > 0 and suite[-1][-1] >= indent:
                        # Pop until the last element with indent less than current indent
                        suite.pop()
                    suite.append([line.strip(), indent])

    return test_status_map |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_chart_js

```
parse_log_chart_js(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated by ChartJS test suite

Source code in `swebench/harness/log_parsers/javascript.py` | 58
59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74 | def parse_log_chart_js(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated by ChartJS test suite
    """
    log = ansi_escape(log)
    test_status_map = {}
    failure_case_patterns = [
        # use [^\S\r\n] to avoid overlapping Chrome groups on separate lines
        (r"Chrome\s[\d\.]+[^\S\r\n]\(.+?\)[^\S\r\n](.*)FAILED$", re.MULTILINE),
    ]
    for failure_case_pattern, flags in failure_case_patterns:
        failures = re.findall(failure_case_pattern, log, flags)
        if len(failures) == 0:
            continue
        for failure in failures:
            test_status_map[failure] = TestStatus.FAILED.value
    return test_status_map |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_marked

```
parse_log_marked(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated by Marked test suite

Source code in `swebench/harness/log_parsers/javascript.py` | 77
78
79
80
81
82
83
84
85
86 | def parse_log_marked(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated by Marked test suite
    """
    test_status_map = {}
    for line in log.split("\n"):
        if re.search(r"^\d+\)\s(.*)", line):
            test = re.search(r"^\d+\)\s(.*)", line).group(1)
            test_status_map[test.strip()] = TestStatus.FAILED.value
    return test_status_map |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_p5js

```
parse_log_p5js(log: str, test_spec: TestSpec) -> dict[str, str]
```

Source code in `swebench/harness/log_parsers/javascript.py` | 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156 | def parse_log_p5js(log: str, test_spec: TestSpec) -> dict[str, str]:
    def remove_json_blocks(log_content):
        filtered_lines = []
        in_json_block = False
        in_json_list_block = False
        for line in log_content.split("\n"):
            stripped_line = line.rstrip()  # Remove trailing whitespace
            if stripped_line.endswith("{"):
                in_json_block = True
                continue
            if stripped_line.endswith("["):
                in_json_list_block = True
                continue
            if stripped_line == "}" and in_json_block:
                in_json_block = False
                continue
            if stripped_line == "]" and in_json_list_block:
                in_json_list_block = False
                continue
            if in_json_block or in_json_list_block:
                continue
            if stripped_line.startswith("{") and stripped_line.endswith("}"):
                continue
            if stripped_line.startswith("[") and stripped_line.endswith("]"):
                continue
            filtered_lines.append(line)
        return "\n".join(filtered_lines)

    def remove_xml_blocks(log_content):
        xml_pat = re.compile(r"<(\w+)>[\s\S]*?<\/\1>", re.MULTILINE)
        match = xml_pat.search(log_content)
        while match:
            # count the number of opening tags in the match
            opening_tags = match.group().count(rf"<{match.group(1)}>") - 1
            opening_tags = max(opening_tags, 0)
            start = match.start()
            end = match.end()
            log_content = (
                log_content[:start]
                + f"<{match.group(1)}>" * opening_tags
                + log_content[end:]
            )
            match = xml_pat.search(log_content)
        return log_content

    def is_valid_fail(match):
        last_line_indent = 0
        for line in match.group(2).split("\n"):
            line_indent = len(line) - len(line.lstrip())
            if line_indent <= last_line_indent:
                return False
            last_line_indent = line_indent
        return True

    log = ansi_escape(log)
    log = remove_json_blocks(log)
    log = remove_xml_blocks(log)
    test_results = {}

    # Parse failing tests
    fail_pattern = re.compile(r"^\s*(\d+)\)(.{0,1000}?):", re.MULTILINE \| re.DOTALL)
    for match in fail_pattern.finditer(log):
        if is_valid_fail(match):
            test_names = list(map(str.strip, match.group(2).split("\n")))
            full_name = ":".join(test_names)
            test_results[full_name] = TestStatus.FAILED.value

    return test_results |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_react_pdf

```
parse_log_react_pdf(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated by Carbon test suite

Source code in `swebench/harness/log_parsers/javascript.py` | 159
160
161
162
163
164
165
166
167
168
169
170
171
172
173
174
175
176
177
178
179 | def parse_log_react_pdf(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated by Carbon test suite
    """
    test_status_map = {}
    for line in log.split("\n"):
        for pattern in [
            (r"^PASS\s(.*)\s\([\d\.]+ms\)", TestStatus.PASSED.value),
            (r"^PASS\s(.*)\s\([\d\.]+\ss\)", TestStatus.PASSED.value),
            (r"^PASS\s(.*)\s\([\d\.]+s\)", TestStatus.PASSED.value),
            (r"^PASS\s(.*)", TestStatus.PASSED.value),
            (r"^FAIL\s(.*)\s\([\d\.]+ms\)", TestStatus.FAILED.value),
            (r"^FAIL\s(.*)\s\([\d\.]+\ss\)", TestStatus.FAILED.value),
            (r"^FAIL\s(.*)\s\([\d\.]+s\)", TestStatus.FAILED.value),
            (r"^FAIL\s(.*)", TestStatus.FAILED.value),
        ]:
            if re.search(pattern[0], line):
                test_name = re.match(pattern[0], line).group(1)
                test_status_map[test_name] = pattern[1]
                break
    return test_status_map |
| ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_jest

```
parse_log_jest(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with Jest. Assumes --verbose flag.

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/javascript.py` | 182
183
184
185
186
187
188
189
190
191
192
193
194
195
196
197
198
199
200
201
202
203
204
205 | def parse_log_jest(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with Jest. Assumes --verbose flag.

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}

    pattern = r"^\s*(✓\|✕\|○)\s(.+?)(?:\s\((\d+\s*m?s)\))?$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status_symbol, test_name, _duration = match.groups()
            if status_symbol == "✓":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status_symbol == "✕":
                test_status_map[test_name] = TestStatus.FAILED.value
            elif status_symbol == "○":
                test_status_map[test_name] = TestStatus.SKIPPED.value
    return test_status_map |
| ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            parse_log_jest_json

```
parse_log_jest_json(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with Jest. Assumes the --json flag has been
piped into JEST_JSON_JQ_TRANSFORM. Unlike --verbose, tests with the same name
in different describe blocks print with different names.

Source code in `swebench/harness/log_parsers/javascript.py` | 208
209
210
211
212
213
214
215
216
217
218
219
220
221
222
223
224
225
226 | def parse_log_jest_json(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with Jest. Assumes the --json flag has been
    piped into JEST_JSON_JQ_TRANSFORM. Unlike --verbose, tests with the same name
    in different describe blocks print with different names.
    """
    test_status_map = {}

    pattern = r"^\[(PASSED\|FAILED)\]\s(.+)$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status, test_name = match.groups()
            if status == "PASSED":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status == "FAILED":
                test_status_map[test_name] = TestStatus.FAILED.value
    return test_status_map |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_vitest

```
parse_log_vitest(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with vitest. Assumes --reporter=verbose flag.

Source code in `swebench/harness/log_parsers/javascript.py` | 229
230
231
232
233
234
235
236
237
238
239
240
241
242
243
244
245
246
247 | def parse_log_vitest(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with vitest. Assumes --reporter=verbose flag.
    """
    test_status_map = {}

    pattern = r"^\s*(✓\|×\|↓)\s(.+?)(?:\s(\d+\s*m?s?\|\[skipped\]))?$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status_symbol, test_name, _duration_or_skipped = match.groups()
            if status_symbol == "✓":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status_symbol == "×":
                test_status_map[test_name] = TestStatus.FAILED.value
            elif status_symbol == "↓":
                test_status_map[test_name] = TestStatus.SKIPPED.value
    return test_status_map |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_karma

```
parse_log_karma(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with Karma. Handles duplicate test names in
different describe blocks. Logic is brittle.

Source code in `swebench/harness/log_parsers/javascript.py` | 250
251
252
253
254
255
256
257
258
259
260
261
262
263
264
265
266
267
268
269
270
271
272
273
274
275
276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292
293
294
295
296 | def parse_log_karma(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with Karma. Handles duplicate test names in
    different describe blocks. Logic is brittle.
    """
    test_status_map = {}
    current_indent = -1
    current_suite = []
    started = False

    pattern = r"^(\s*)?([✔✖])?\s(.*)$"

    for line in log.split("\n"):
        if line.startswith("SUMMARY:"):
            # Individual test logs end here
            return test_status_map

        if "Starting browser" in line:
            started = True
            continue

        if not started:
            continue

        match = re.match(pattern, line)
        if match:
            indent, status, name = match.groups()

            if indent and not status:
                new_indent = len(indent)
                if new_indent > current_indent:
                    current_indent = new_indent
                    current_suite.append(name)
                elif new_indent < current_indent:
                    current_indent = new_indent
                    current_suite.pop()
                    continue

            if status in ("✔", "✖"):
                full_test_name = " > ".join(current_suite + [name])
                test_status_map[full_test_name] = (
                    TestStatus.PASSED.value
                    if status == "✔"
                    else TestStatus.FAILED.value
                )

    return test_status_map |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_tap

```
parse_log_tap(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with TAP

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/javascript.py` | 299
300
301
302
303
304
305
306
307
308
309
310
311
312
313
314
315
316
317
318
319
320
321
322 | def parse_log_tap(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with TAP

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}

    # Pattern to match TAP result lines
    pattern = r"^(ok\|not ok) (\d+) (.+)$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            status, _test_number, test_name = match.groups()
            if status == "ok":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif status == "not ok":
                test_status_map[test_name] = TestStatus.FAILED.value

    return test_status_map |
| ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_immutable_js

```
parse_log_immutable_js(log: str, test_spec: TestSpec) -> dict[str, str]
```

Different immutable.js instances use different test runners and log formats.
This function selects the appropriate log parser based on the instance id.

Source code in `swebench/harness/log_parsers/javascript.py` | 325
326
327
328
329
330
331
332
333
334
335
336
337 | def parse_log_immutable_js(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Different immutable.js instances use different test runners and log formats.
    This function selects the appropriate log parser based on the instance id.
    """
    pr_number = test_spec.instance_id.split("-")[-1]

    if pr_number in ["2006"]:
        return parse_log_jest(log, test_spec)
    elif pr_number in ["2005"]:
        return parse_log_jest_json(log, test_spec)
    else:
        raise ValueError(f"Unknown instance id: {test_spec.instance_id}") |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            php

######
            MAP_REPO_TO_PARSER_PHP

  `module-attribute`

```
MAP_REPO_TO_PARSER_PHP = {'phpoffice/phpspreadsheet': parse_log_phpunit, 'laravel/framework': parse_log_phpunit, 'php-cs-fixer/php-cs-fixer': parse_log_phpunit, 'briannesbitt/carbon': parse_log_phpunit}
```

######
            parse_log_phpunit

```
parse_log_phpunit(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for phpunit logs with the --testdox option.
Args:
    log (str): log content
    test_spec (TestSpec): test spec (unused)
Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/php.py` | 6
 7
 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39 | def parse_log_phpunit(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for phpunit logs with the --testdox option.
    Args:
        log (str): log content
        test_spec (TestSpec): test spec (unused)
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}
    suite = None

    suite_pattern = r"^(\w.+) \(.+\)$"
    test_pattern = r"^\s*([✔✘↩])\s*(.*)$"

    for line in log.split("\n"):
        suite_match = re.match(suite_pattern, line)
        if suite_match:
            suite = suite_match.groups()[0]
            continue

        test_match = re.match(test_pattern, line)
        if test_match:
            status, test_name = test_match.groups()
            full_test_name = f"{suite} > {test_name}"

            if status == "✔":
                test_status_map[full_test_name] = TestStatus.PASSED.value
            elif status == "✘":
                test_status_map[full_test_name] = TestStatus.FAILED.value
            elif status == "↩":
                test_status_map[full_test_name] = TestStatus.SKIPPED.value

    return test_status_map |
| ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            python

######
            parse_log_astroid

  `module-attribute`

```
parse_log_astroid = parse_log_pytest
```

######
            parse_log_flask

  `module-attribute`

```
parse_log_flask = parse_log_pytest
```

######
            parse_log_marshmallow

  `module-attribute`

```
parse_log_marshmallow = parse_log_pytest
```

######
            parse_log_pvlib

  `module-attribute`

```
parse_log_pvlib = parse_log_pytest
```

######
            parse_log_pyvista

  `module-attribute`

```
parse_log_pyvista = parse_log_pytest
```

######
            parse_log_sqlfluff

  `module-attribute`

```
parse_log_sqlfluff = parse_log_pytest
```

######
            parse_log_xarray

  `module-attribute`

```
parse_log_xarray = parse_log_pytest
```

######
            parse_log_pydicom

  `module-attribute`

```
parse_log_pydicom = parse_log_pytest_options
```

######
            parse_log_requests

  `module-attribute`

```
parse_log_requests = parse_log_pytest_options
```

######
            parse_log_pylint

  `module-attribute`

```
parse_log_pylint = parse_log_pytest_options
```

######
            parse_log_astropy

  `module-attribute`

```
parse_log_astropy = parse_log_pytest_v2
```

######
            parse_log_scikit

  `module-attribute`

```
parse_log_scikit = parse_log_pytest_v2
```

######
            parse_log_sphinx

  `module-attribute`

```
parse_log_sphinx = parse_log_pytest_v2
```

######
            MAP_REPO_TO_PARSER_PY

  `module-attribute`

```
MAP_REPO_TO_PARSER_PY = {'astropy/astropy': parse_log_astropy, 'django/django': parse_log_django, 'marshmallow-code/marshmallow': parse_log_marshmallow, 'matplotlib/matplotlib': parse_log_matplotlib, 'mwaskom/seaborn': parse_log_seaborn, 'pallets/flask': parse_log_flask, 'psf/requests': parse_log_requests, 'pvlib/pvlib-python': parse_log_pvlib, 'pydata/xarray': parse_log_xarray, 'pydicom/pydicom': parse_log_pydicom, 'pylint-dev/astroid': parse_log_astroid, 'pylint-dev/pylint': parse_log_pylint, 'pytest-dev/pytest': parse_log_pytest, 'pyvista/pyvista': parse_log_pyvista, 'scikit-learn/scikit-learn': parse_log_scikit, 'sqlfluff/sqlfluff': parse_log_sqlfluff, 'sphinx-doc/sphinx': parse_log_sphinx, 'sympy/sympy': parse_log_sympy}
```

######
            parse_log_pytest

```
parse_log_pytest(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with PyTest framework

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/python.py` | 7
 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26 | def parse_log_pytest(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with PyTest framework

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}
    for line in log.split("\n"):
        if any([line.startswith(x.value) for x in TestStatus]):
            # Additional parsing for FAILED status
            if line.startswith(TestStatus.FAILED.value):
                line = line.replace(" - ", " ")
            test_case = line.split()
            if len(test_case) <= 1:
                continue
            test_status_map[test_case[1]] = test_case[0]
    return test_status_map |
| ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_pytest_options

```
parse_log_pytest_options(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with PyTest framework with options

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/python.py` | 29
30
31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61 | def parse_log_pytest_options(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with PyTest framework with options

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    option_pattern = re.compile(r"(.*?)\[(.*)\]")
    test_status_map = {}
    for line in log.split("\n"):
        if any([line.startswith(x.value) for x in TestStatus]):
            # Additional parsing for FAILED status
            if line.startswith(TestStatus.FAILED.value):
                line = line.replace(" - ", " ")
            test_case = line.split()
            if len(test_case) <= 1:
                continue
            has_option = option_pattern.search(test_case[1])
            if has_option:
                main, option = has_option.groups()
                if (
                    option.startswith("/")
                    and not option.startswith("//")
                    and "*" not in option
                ):
                    option = "/" + option.split("/")[-1]
                test_name = f"{main}[{option}]"
            else:
                test_name = test_case[1]
            test_status_map[test_name] = test_case[0]
    return test_status_map |
| -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_django

```
parse_log_django(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with Django tester framework

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/python.py` | 64
 65
 66
 67
 68
 69
 70
 71
 72
 73
 74
 75
 76
 77
 78
 79
 80
 81
 82
 83
 84
 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141 | def parse_log_django(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with Django tester framework

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}
    lines = log.split("\n")

    prev_test = None
    for line in lines:
        line = line.strip()

        # This isn't ideal but the test output spans multiple lines
        if "--version is equivalent to version" in line:
            test_status_map["--version is equivalent to version"] = (
                TestStatus.PASSED.value
            )

        # Log it in case of error
        if " ... " in line:
            prev_test = line.split(" ... ")[0]

        pass_suffixes = (" ... ok", " ... OK", " ...  OK")
        for suffix in pass_suffixes:
            if line.endswith(suffix):
                # TODO: Temporary, exclusive fix for django__django-7188
                # The proper fix should involve somehow getting the test results to
                # print on a separate line, rather than the same line
                if line.strip().startswith(
                    "Applying sites.0002_alter_domain_unique...test_no_migrations"
                ):
                    line = line.split("...", 1)[-1].strip()
                test = line.rsplit(suffix, 1)[0]
                test_status_map[test] = TestStatus.PASSED.value
                break
        if " ... skipped" in line:
            test = line.split(" ... skipped")[0]
            test_status_map[test] = TestStatus.SKIPPED.value
        if line.endswith(" ... FAIL"):
            test = line.split(" ... FAIL")[0]
            test_status_map[test] = TestStatus.FAILED.value
        if line.startswith("FAIL:"):
            test = line.split()[1].strip()
            test_status_map[test] = TestStatus.FAILED.value
        if line.endswith(" ... ERROR"):
            test = line.split(" ... ERROR")[0]
            test_status_map[test] = TestStatus.ERROR.value
        if line.startswith("ERROR:"):
            test = line.split()[1].strip()
            test_status_map[test] = TestStatus.ERROR.value

        if line.lstrip().startswith("ok") and prev_test is not None:
            # It means the test passed, but there's some additional output (including new lines)
            # between "..." and "ok" message
            test = prev_test
            test_status_map[test] = TestStatus.PASSED.value

    # TODO: This is very brittle, we should do better
    # There's a bug in the django logger, such that sometimes a test output near the end gets
    # interrupted by a particular long multiline print statement.
    # We have observed this in one of 3 forms:
    # - "{test_name} ... Testing against Django installed in {*} silenced.\nok"
    # - "{test_name} ... Internal Server Error: \/(.*)\/\nok"
    # - "{test_name} ... System check identified no issues (0 silenced).\nok"
    patterns = [
        r"^(.*?)\s\.\.\.\sTesting\ against\ Django\ installed\ in\ ((?s:.*?))\ silenced\)\.\nok$",
        r"^(.*?)\s\.\.\.\sInternal\ Server\ Error:\ \/(.*)\/\nok$",
        r"^(.*?)\s\.\.\.\sSystem check identified no issues \(0 silenced\)\nok$",
    ]
    for pattern in patterns:
        for match in re.finditer(pattern, log, re.MULTILINE):
            test_name = match.group(1)
            test_status_map[test_name] = TestStatus.PASSED.value
    return test_status_map |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_pytest_v2

```
parse_log_pytest_v2(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with PyTest framework (Later Version)

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/python.py` | 144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159
160
161
162
163
164
165
166
167
168
169
170 | def parse_log_pytest_v2(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with PyTest framework (Later Version)

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}
    escapes = "".join([chr(char) for char in range(1, 32)])
    for line in log.split("\n"):
        line = re.sub(r"\[(\d+)m", "", line)
        translator = str.maketrans("", "", escapes)
        line = line.translate(translator)
        if any([line.startswith(x.value) for x in TestStatus]):
            if line.startswith(TestStatus.FAILED.value):
                line = line.replace(" - ", " ")
            test_case = line.split()
            if len(test_case) >= 2:
                test_status_map[test_case[1]] = test_case[0]
        # Support older pytest versions by checking if the line ends with the test status
        elif any([line.endswith(x.value) for x in TestStatus]):
            test_case = line.split()
            if len(test_case) >= 2:
                test_status_map[test_case[0]] = test_case[1]
    return test_status_map |
| ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_seaborn

```
parse_log_seaborn(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with seaborn testing framework

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/python.py` | 173
174
175
176
177
178
179
180
181
182
183
184
185
186
187
188
189
190
191
192
193
194
195
196 | def parse_log_seaborn(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with seaborn testing framework

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}
    for line in log.split("\n"):
        if line.startswith(TestStatus.FAILED.value):
            test_case = line.split()[1]
            test_status_map[test_case] = TestStatus.FAILED.value
        elif f" {TestStatus.PASSED.value} " in line:
            parts = line.split()
            if parts[1] == TestStatus.PASSED.value:
                test_case = parts[0]
                test_status_map[test_case] = TestStatus.PASSED.value
        elif line.startswith(TestStatus.PASSED.value):
            parts = line.split()
            test_case = parts[1]
            test_status_map[test_case] = TestStatus.PASSED.value
    return test_status_map |
| ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_sympy

```
parse_log_sympy(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with Sympy framework

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/python.py` | 199
200
201
202
203
204
205
206
207
208
209
210
211
212
213
214
215
216
217
218
219
220
221
222
223
224
225
226 | def parse_log_sympy(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with Sympy framework

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}
    pattern = r"(_*) (.*)\.py:(.*) (_*)"
    matches = re.findall(pattern, log)
    for match in matches:
        test_case = f"{match[1]}.py:{match[2]}"
        test_status_map[test_case] = TestStatus.FAILED.value
    for line in log.split("\n"):
        line = line.strip()
        if line.startswith("test_"):
            if line.endswith(" E"):
                test = line.split()[0]
                test_status_map[test] = TestStatus.ERROR.value
            if line.endswith(" F"):
                test = line.split()[0]
                test_status_map[test] = TestStatus.FAILED.value
            if line.endswith(" ok"):
                test = line.split()[0]
                test_status_map[test] = TestStatus.PASSED.value
    return test_status_map |
| --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_matplotlib

```
parse_log_matplotlib(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parser for test logs generated with PyTest framework

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/python.py` | 229
230
231
232
233
234
235
236
237
238
239
240
241
242
243
244
245
246
247
248
249
250 | def parse_log_matplotlib(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Parser for test logs generated with PyTest framework

    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}
    for line in log.split("\n"):
        line = line.replace("MouseButton.LEFT", "1")
        line = line.replace("MouseButton.RIGHT", "3")
        if any([line.startswith(x.value) for x in TestStatus]):
            # Additional parsing for FAILED status
            if line.startswith(TestStatus.FAILED.value):
                line = line.replace(" - ", " ")
            test_case = line.split()
            if len(test_case) <= 1:
                continue
            test_status_map[test_case[1]] = test_case[0]
    return test_status_map |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

#####
            ruby

######
            MAP_REPO_TO_PARSER_RUBY

  `module-attribute`

```
MAP_REPO_TO_PARSER_RUBY = {'jekyll/jekyll': parse_log_jekyll, 'fluent/fluentd': parse_log_ruby_unit, 'fastlane/fastlane': parse_log_rspec_transformed_json, 'jordansissel/fpm': parse_log_rspec_transformed_json, 'faker-ruby/faker': parse_log_ruby_unit, 'rubocop/rubocop': parse_log_rspec_transformed_json}
```

######
            parse_log_minitest

```
parse_log_minitest(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/ruby.py` | 7
 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27 | def parse_log_minitest(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}

    pattern = r"^(.+)\. .*=.*(\.\|F\|E).*$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            test_name, outcome = match.groups()
            if outcome == ".":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif outcome in ["F", "E"]:
                test_status_map[test_name] = TestStatus.FAILED.value

    return test_status_map |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_cucumber

```
parse_log_cucumber(log: str, test_spec: TestSpec) -> dict[str, str]
```

Assumes --format progress is used.

Source code in `swebench/harness/log_parsers/ruby.py` | 30
31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47 | def parse_log_cucumber(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Assumes --format progress is used.
    """
    test_status_map = {}

    pattern = r"^(.*) \.+(\.\|F)"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            test_name, outcome = match.groups()
            if outcome == ".":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif outcome == "F":
                test_status_map[test_name] = TestStatus.FAILED.value

    return test_status_map |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_ruby_unit

```
parse_log_ruby_unit(log: str, test_spec: TestSpec) -> dict[str, str]
```

Source code in `swebench/harness/log_parsers/ruby.py` | 50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65
66 | def parse_log_ruby_unit(log: str, test_spec: TestSpec) -> dict[str, str]:
    test_status_map = {}

    pattern = r"^\s*(?:test: )?(.+):\s+(\.\|E\b\|F\b\|O\b)"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            test_name, outcome = match.groups()
            if outcome == ".":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif outcome in ["E", "F"]:
                test_status_map[test_name] = TestStatus.FAILED.value
            elif outcome == "O":
                test_status_map[test_name] = TestStatus.SKIPPED.value

    return test_status_map |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            parse_log_rspec_transformed_json

```
parse_log_rspec_transformed_json(log: str, test_spec: TestSpec) -> dict[str, str]
```

Source code in `swebench/harness/log_parsers/ruby.py` | 69
70
71
72
73
74
75
76
77
78
79
80
81
82
83
84
85
86
87 | def parse_log_rspec_transformed_json(log: str, test_spec: TestSpec) -> dict[str, str]:
    test_status_map = {}

    pattern = r"(.+) - (passed\|failed)"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            test_name, outcome = match.groups()
            if outcome == "passed":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif outcome == "failed":
                test_status_map[test_name] = TestStatus.FAILED.value
            elif outcome == "pending":
                test_status_map[test_name] = TestStatus.SKIPPED.value
            else:
                raise ValueError(f"Unknown outcome: {outcome}")

    return test_status_map |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            parse_log_jekyll

```
parse_log_jekyll(log: str, test_spec: TestSpec) -> dict[str, str]
```

Different jekyll instances use different test runners and log formats.
This function selects the appropriate log parser based on the instance id.

Source code in `swebench/harness/log_parsers/ruby.py` | 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102 | def parse_log_jekyll(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Different jekyll instances use different test runners and log formats.
    This function selects the appropriate log parser based on the instance id.
    """
    pr_number = test_spec.instance_id.split("-")[1]

    if pr_number in ["9141", "8047", "8167"]:
        return parse_log_minitest(log, test_spec)
    elif pr_number in ["8761", "8771"]:
        return parse_log_cucumber(log, test_spec)
    else:
        raise ValueError(f"Unknown instance id: {test_spec.instance_id}") |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            rust

######
            MAP_REPO_TO_PARSER_RUST

  `module-attribute`

```
MAP_REPO_TO_PARSER_RUST = {'burntsushi/ripgrep': parse_log_cargo, 'sharkdp/bat': parse_log_cargo, 'astral-sh/ruff': parse_log_cargo, 'tokio-rs/tokio': parse_log_cargo, 'uutils/coreutils': parse_log_cargo, 'nushell/nushell': parse_log_cargo, 'tokio-rs/axum': parse_log_cargo}
```

######
            parse_log_cargo

```
parse_log_cargo(log: str, test_spec: TestSpec) -> dict[str, str]
```

Parameters:

| Name | Type | Description | Default  |
| ---- | ---- | ----------- | -------- |
| log  | str  | log content | required |

Returns:
    dict: test case to test status mapping

Source code in `swebench/harness/log_parsers/rust.py` | 7
 8
 9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27 | def parse_log_cargo(log: str, test_spec: TestSpec) -> dict[str, str]:
    """
    Args:
        log (str): log content
    Returns:
        dict: test case to test status mapping
    """
    test_status_map = {}

    pattern = r"^test\s+(\S+)\s+\.\.\.\s+(\w+)$"

    for line in log.split("\n"):
        match = re.match(pattern, line.strip())
        if match:
            test_name, outcome = match.groups()
            if outcome == "ok":
                test_status_map[test_name] = TestStatus.PASSED.value
            elif outcome == "FAILED":
                test_status_map[test_name] = TestStatus.FAILED.value

    return test_status_map |
| ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            modal_eval

#####
            __all__

  `module-attribute`

```
__all__ = ['run_instances_modal', 'validate_modal_credentials']
```

#####
            run_instances_modal

```
run_instances_modal(predictions: dict, instances: list, full_dataset: list, run_id: str, timeout: int)
```

Run all instances for the given predictions on Modal.

Parameters:

| Name        | Type | Description                             | Default  |
| ----------- | ---- | --------------------------------------- | -------- |
| predictions | dict | Predictions dict generated by the model | required |
| instances   | list | List of instances                       | required |
| run_id      | str  | Run ID                                  | required |
| timeout     | int  | Timeout for running tests               | required |

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 394
395
396
397
398
399
400
401
402
403
404
405
406
407
408
409
410
411
412
413
414
415
416
417
418
419
420
421
422
423
424
425
426
427
428
429
430
431
432
433
434
435
436
437
438
439
440
441
442
443
444
445
446
447
448
449
450
451
452
453
454
455
456
457
458
459
460
461
462 | def run_instances_modal(
    predictions: dict,
    instances: list,
    full_dataset: list,
    run_id: str,
    timeout: int,
):
    """
    Run all instances for the given predictions on Modal.

    Args:
        predictions (dict): Predictions dict generated by the model
        instances (list): List of instances
        run_id (str): Run ID
        timeout (int): Timeout for running tests
    """
    test_specs = list(map(make_test_spec, instances))

    with modal.enable_output():
        with app.run():
            run_test_specs = []

            # Check for instances that have already been run
            for test_spec in test_specs:
                log_dir = get_log_dir(
                    predictions[test_spec.instance_id], run_id, test_spec.instance_id
                )
                if log_dir.exists():
                    continue
                run_test_specs.append(test_spec)

            if run_test_specs:
                # Run instances that haven't been run yet
                results = run_instance_modal.starmap(
                    [
                        (
                            test_spec,
                            predictions[test_spec.instance_id],
                            run_id,
                            timeout,
                        )
                        for test_spec in run_test_specs
                    ],
                    return_exceptions=True,
                )

                for result in results:
                    if not isinstance(result, TestOutput):
                        print(f"Result failed with error: {result}")
                        continue

                    # Save logs locally
                    log_dir = result.log_dir
                    log_dir.mkdir(parents=True, exist_ok=True)
                    with open(log_dir / "run_instance.log", "w") as f:
                        f.write(result.run_instance_log)
                    with open(log_dir / "test_output.txt", "w") as f:
                        f.write(result.test_output)
                    with open(log_dir / "patch.diff", "w") as f:
                        f.write(result.patch_diff)
                    with open(log_dir / "report.json", "w") as f:
                        try:
                            report_json = json.loads(result.report_json_str)
                            json.dump(report_json, f, indent=4)
                        except Exception:
                            # This happens if the test fails with any exception
                            print(f"{result.instance_id}: no report.json")

            make_run_report(predictions, full_dataset, run_id) |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            validate_modal_credentials

```
validate_modal_credentials()
```

Validate that Modal credentials exist by checking for ~/.modal.toml file.
Raises an exception if credentials are not configured.

Source code in `swebench/harness/modal_eval/utils.py` | 4
 5
 6
 7
 8
 9
10
11
12
13
14 | def validate_modal_credentials():
    """
    Validate that Modal credentials exist by checking for ~/.modal.toml file.
    Raises an exception if credentials are not configured.
    """
    modal_config_path = Path.home() / ".modal.toml"
    if not modal_config_path.exists():
        raise RuntimeError(
            "~/.modal.toml not found - it looks like you haven't configured credentials for Modal.\n"
            "Run 'modal token new' in your terminal to configure credentials."
        ) |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            run_evaluation_modal

######
            SANDBOX_ENTRYPOINT

  `module-attribute`

```
SANDBOX_ENTRYPOINT = 'run_evaluation_modal_entrypoint'
```

######
            LOCAL_SANDBOX_ENTRYPOINT_PATH

  `module-attribute`

```
LOCAL_SANDBOX_ENTRYPOINT_PATH = resolve()
```

######
            REMOTE_SANDBOX_ENTRYPOINT_PATH

  `module-attribute`

```
REMOTE_SANDBOX_ENTRYPOINT_PATH = f'/root/{SANDBOX_ENTRYPOINT}.py'
```

######
            app

  `module-attribute`

```
app = App('swebench-evaluation')
```

######
            swebench_image

  `module-attribute`

```
swebench_image = pip_install('swebench', 'tenacity')
```

######
            TestOutput

  `dataclass`

```
TestOutput(instance_id: str, test_output: str, report_json_str: str, run_instance_log: str, patch_diff: str, log_dir: Path, errored: bool)
```

instance_id `instance-attribute` ```
instance_id: str
```

test_output `instance-attribute` ```
test_output: str
```

report_json_str `instance-attribute` ```
report_json_str: str
```

run_instance_log `instance-attribute` ```
run_instance_log: str
```

patch_diff `instance-attribute` ```
patch_diff: str
```

log_dir `instance-attribute` ```
log_dir: Path
```

errored `instance-attribute` ```
errored: bool
```

######
            ModalSandboxRuntime

```
ModalSandboxRuntime(test_spec: TestSpec, timeout: int | None = None, verbose: bool = True)
```

Runtime for running instances in a Modal Sandbox.

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 56
57
58
59
60
61
62
63
64
65
66 | def __init__(
    self, test_spec: TestSpec, timeout: int \| None = None, verbose: bool = True
):
    self.test_spec = test_spec
    self.image = ModalSandboxRuntime.get_instance_image(test_spec)
    self.sandbox = self._get_sandbox(timeout)
    self.verbose = verbose
    self._stream_tasks = []

    # Hack for pylint
    self.write_file("/sys/fs/cgroup/cpu/cpu.shares", "2048") |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

test_spec `instance-attribute` ```
test_spec = test_spec
```

image `instance-attribute` ```
image = get_instance_image(test_spec)
```

sandbox `instance-attribute` ```
sandbox = _get_sandbox(timeout)
```

verbose `instance-attribute` ```
verbose = verbose
```

write_file ```
write_file(file_path: str, content: str)
```

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 116
117 | def write_file(self, file_path: str, content: str):
    self.sandbox.open(file_path, "w").write(content) |
| ------- | -------------------------------------------------------------------------------------------------------- |

exec ```
exec(command: str) -> tuple[str, int]
```

Execute a command in the sandbox.

Returns:

| Type            | Description                                      |
| --------------- | ------------------------------------------------ |
| tuple[str, int] | tuple[str, int]: Sandbox output and return code. |

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137 | def exec(self, command: str) -> tuple[str, int]:
    """
    Execute a command in the sandbox.

    Returns:
        tuple[str, int]: Sandbox output and return code.
    """
    p = self.sandbox.exec("python", "-m", SANDBOX_ENTRYPOINT, command)
    stdout = []
    stderr = []
    try:
        # We separate stdout/stderr because some tests rely on them being separate.
        # We still read stdout/stderr simultaneously to continuously
        # flush both streams and avoid blocking.
        asyncio.run(self._read_output(p, stdout, stderr))
    except Exception as e:
        print(f"Error during command execution: {e}")
    p.wait()
    return "".join(stdout + stderr), p.returncode |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

__exit__ ```
__exit__(exc_type, exc_val, exc_tb)
```

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157 | def __exit__(self, exc_type, exc_val, exc_tb):
    if self._stream_tasks:
        try:
            # Forcefully kill remaining streams
            for task in self._stream_tasks:
                if not task.done():
                    task.cancel()
                    try:
                        asyncio.wait_for(task, timeout=0.1)
                    except asyncio.TimeoutError:
                        pass
                    except Exception:
                        pass

            self.sandbox.terminate()
        except Exception:
            pass
        finally:
            self._stream_tasks = [] |
| --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

get_instance_image `staticmethod` ```
get_instance_image(test_spec: TestSpec) -> Image
```

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 159
160
161
162
163
164
165
166
167
168
169
170
171
172
173
174
175
176
177
178
179
180
181
182
183
184
185
186
187
188
189
190
191
192
193
194
195
196
197
198
199
200
201
202
203
204
205
206
207
208
209
210
211
212
213
214 | @staticmethod
def get_instance_image(test_spec: TestSpec) -> modal.Image:
    env_script = test_spec.setup_env_script
    # add trusted host flag for Modal's PyPI mirror
    env_script = env_script.replace(
        "conda activate testbed && python -m pip install -r $HOME/requirements.txt",
        "conda activate testbed && python -m pip install --trusted-host pypi-mirror.modal.local -r $HOME/requirements.txt",
    )
    repo_script = test_spec.install_repo_script

    remote_env_script_path = "/root/setup_env.sh"
    remote_repo_script_path = "/root/setup_repo.sh"

    Path(remote_env_script_path).write_text(env_script)
    Path(remote_repo_script_path).write_text(repo_script)

    # Modal automatically caches images
    # https://modal.com/docs/guide/custom-container#image-caching-and-rebuilds
    return (
        modal.Image.from_registry("ubuntu:22.04", add_python="3.11")
        .run_commands("apt update")
        .env({"DEBIAN_FRONTEND": "noninteractive", "TZ": "Etc/UTC"})
        .apt_install(
            "wget",
            "git",
            "build-essential",
            "libffi-dev",
            "libtiff-dev",
            "jq",
            "curl",
            "locales",
            "locales-all",
            "tzdata",
        )
        .run_commands(
            "wget 'https://repo.anaconda.com/miniconda/Miniconda3-py311_23.11.0-2-Linux-x86_64.sh' -O miniconda.sh",
            "bash miniconda.sh -b -p /opt/miniconda3",
            "echo 'export PATH=/opt/miniconda3/bin:$PATH' >> ~/.bashrc",
            "/opt/miniconda3/bin/conda init --all",
            "/opt/miniconda3/bin/conda config --append channels conda-forge",
            "adduser --disabled-password --gecos 'dog' nonroot",
        )
        .add_local_file(
            Path(remote_env_script_path), remote_env_script_path, copy=True
        )
        .add_local_file(
            Path(remote_repo_script_path), remote_repo_script_path, copy=True
        )
        .run_commands(
            f"chmod +x {remote_env_script_path}",
            f"/bin/bash -c 'source ~/.bashrc && {remote_env_script_path}'",
            "echo 'source /opt/miniconda3/etc/profile.d/conda.sh && conda activate testbed' >> /root/.bashrc",
            f"/bin/bash {remote_repo_script_path}",
        )
        .workdir("/testbed/")
    ) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            get_log_dir

```
get_log_dir(pred: dict, run_id: str, instance_id: str) -> Path
```

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 217
218
219
220
221 | def get_log_dir(pred: dict, run_id: str, instance_id: str) -> Path:
    model_name_or_path = cast(
        str, pred.get("model_name_or_path", "None").replace("/", "__")
    )
    return RUN_EVALUATION_LOG_DIR / run_id / model_name_or_path / instance_id |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            run_instance_modal

```
run_instance_modal(test_spec: TestSpec, pred: dict, run_id: str, timeout: int | None = None) -> TestOutput
```

Run a single instance with the given prediction.

Parameters:

| Name      | Type     | Description                                                | Default  |
| --------- | -------- | ---------------------------------------------------------- | -------- |
| test_spec | TestSpec | TestSpec instance                                          | required |
| pred      | dict     | Prediction w/ model_name_or_path, model_patch, instance_id | required |
| run_id    | str      | Run ID                                                     | required |
| timeout   | int      | Timeout for running tests                                  | None     |

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 224
225
226
227
228
229
230
231
232
233
234
235
236
237
238
239
240
241
242
243
244
245
246
247
248
249
250
251
252
253
254
255
256
257
258
259
260
261
262
263
264
265
266
267
268
269
270
271
272
273
274
275
276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292
293
294
295
296
297
298
299
300
301
302
303
304
305
306
307
308
309
310
311
312
313
314
315
316
317
318
319
320
321
322
323
324
325
326
327
328
329
330
331
332
333
334
335
336
337
338
339
340
341
342
343
344
345
346
347
348
349
350
351
352
353
354
355
356
357
358
359
360
361
362
363
364
365
366
367
368
369
370
371
372
373
374
375
376
377
378
379
380
381
382
383
384
385
386
387
388
389
390
391 | @app.function(
    image=swebench_image.add_local_file(
        LOCAL_SANDBOX_ENTRYPOINT_PATH,
        REMOTE_SANDBOX_ENTRYPOINT_PATH,
    ),
    timeout=120
    * 60,  # Much larger than default timeout to account for image build time
    include_source=True,
)
def run_instance_modal(
    test_spec: TestSpec,
    pred: dict,
    run_id: str,
    timeout: int \| None = None,
) -> TestOutput:
    """
    Run a single instance with the given prediction.

    Args:
        test_spec (TestSpec): TestSpec instance
        pred (dict): Prediction w/ model_name_or_path, model_patch, instance_id
        run_id (str): Run ID
        timeout (int): Timeout for running tests
    """
    instance_id = test_spec.instance_id
    log_dir = get_log_dir(pred, run_id, instance_id)
    log_dir.mkdir(parents=True, exist_ok=True)

    log_file = log_dir / "run_instance.log"

    logger = setup_logger(instance_id, log_file, add_stdout=True)

    try:
        runner = ModalSandboxRuntime(test_spec, timeout)
    except Exception as e:
        print(f"Error creating sandbox: {e}")
        raise EvaluationError(
            instance_id,
            f"Error creating sandbox: {e}",
            logger,
        ) from e

    patch_diff = pred.get("model_patch", "")

    try:
        patch_file = "/tmp/patch.diff"
        runner.write_file(patch_file, patch_diff)

        apply_patch_output, returncode = runner.exec(
            "cd /testbed && git apply -v /tmp/patch.diff",
        )

        if returncode != 0:
            logger.info("Failed to apply patch to container, trying again...")

            apply_patch_output, returncode = runner.exec(
                "cd /testbed && patch --batch --fuzz=5 -p1 -i /tmp/patch.diff",
            )

            if returncode != 0:
                logger.info(f"{APPLY_PATCH_FAIL}:\n{apply_patch_output}")
                raise EvaluationError(
                    instance_id,
                    f"{APPLY_PATCH_FAIL}:\n{apply_patch_output}",
                    logger,
                )
            else:
                logger.info(f"{APPLY_PATCH_PASS}:\n{apply_patch_output}")
        else:
            logger.info(f"{APPLY_PATCH_PASS}:\n{apply_patch_output}")

        # Get git diff before running eval script
        git_diff_output_before, returncode = runner.exec(
            "cd /testbed && git diff",
        )
        logger.info(f"Git diff before:\n{git_diff_output_before}")

        eval_file = "/root/eval.sh"
        eval_script = test_spec.eval_script
        # django hack
        eval_script = eval_script.replace("locale-gen", "locale-gen en_US.UTF-8")
        runner.write_file(eval_file, eval_script)

        start_time = time.time()

        run_command = "cd /testbed"
        # pylint hack
        if "pylint" in test_spec.instance_id:
            run_command += " && PYTHONPATH="
        # increase recursion limit for testing
        run_command += " && python3 -c 'import sys; sys.setrecursionlimit(10000)'"
        # run eval script
        run_command += " && /bin/bash /root/eval.sh"
        test_output, returncode = runner.exec(run_command)

        total_runtime = time.time() - start_time

        test_output_path = log_dir / "test_output.txt"
        logger.info(f"Test runtime: {total_runtime:_.2f} seconds")
        with open(test_output_path, "w") as f:
            f.write(test_output)
            logger.info(f"Test output for {instance_id} written to {test_output_path}")
            print(f"Test output for {instance_id} written to {test_output_path}")

        # Get git diff after running eval script
        git_diff_output_after, returncode = runner.exec("cd /testbed && git diff")

        # Check if git diff changed after running eval script
        logger.info(f"Git diff after:\n{git_diff_output_after}")
        if git_diff_output_after != git_diff_output_before:
            logger.info("Git diff changed after running eval script")

        # Get report from test output
        logger.info(f"Grading answer for {instance_id}...")
        report = get_eval_report(
            test_spec=test_spec,
            prediction=pred,
            test_log_path=test_output_path,
            include_tests_status=True,
        )
        logger.info(
            f"report: {report}\n"
            f"Result for {instance_id}: resolved: {report[instance_id]['resolved']}"
        )

        return TestOutput(
            instance_id=instance_id,
            test_output=test_output,
            report_json_str=json.dumps(report, indent=4),
            run_instance_log=log_file.read_text(),
            patch_diff=patch_diff,
            log_dir=log_dir,
            errored=False,
        )
    except modal.exception.SandboxTimeoutError as e:
        raise EvaluationError(
            instance_id,
            f"Test timed out after {timeout} seconds.",
            logger,
        ) from e
    except EvaluationError:
        error_msg = traceback.format_exc()
        logger.info(error_msg)
        return TestOutput(
            instance_id=instance_id,
            test_output="",
            report_json_str="",
            run_instance_log=log_file.read_text(),
            patch_diff=patch_diff,
            log_dir=log_dir,
            errored=True,
        )
    except Exception as e:
        error_msg = (
            f"Error in evaluating model for {instance_id}: {e}\n"
            f"{traceback.format_exc()}\n"
            f"Check ({logger.log_file}) for more information."
        )
        logger.error(error_msg)
        return TestOutput(
            instance_id=instance_id,
            test_output="",
            report_json_str="",
            run_instance_log=log_file.read_text(),
            patch_diff=patch_diff,
            log_dir=log_dir,
            errored=True,
        ) |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            run_instances_modal

```
run_instances_modal(predictions: dict, instances: list, full_dataset: list, run_id: str, timeout: int)
```

Run all instances for the given predictions on Modal.

Parameters:

| Name        | Type | Description                             | Default  |
| ----------- | ---- | --------------------------------------- | -------- |
| predictions | dict | Predictions dict generated by the model | required |
| instances   | list | List of instances                       | required |
| run_id      | str  | Run ID                                  | required |
| timeout     | int  | Timeout for running tests               | required |

Source code in `swebench/harness/modal_eval/run_evaluation_modal.py` | 394
395
396
397
398
399
400
401
402
403
404
405
406
407
408
409
410
411
412
413
414
415
416
417
418
419
420
421
422
423
424
425
426
427
428
429
430
431
432
433
434
435
436
437
438
439
440
441
442
443
444
445
446
447
448
449
450
451
452
453
454
455
456
457
458
459
460
461
462 | def run_instances_modal(
    predictions: dict,
    instances: list,
    full_dataset: list,
    run_id: str,
    timeout: int,
):
    """
    Run all instances for the given predictions on Modal.

    Args:
        predictions (dict): Predictions dict generated by the model
        instances (list): List of instances
        run_id (str): Run ID
        timeout (int): Timeout for running tests
    """
    test_specs = list(map(make_test_spec, instances))

    with modal.enable_output():
        with app.run():
            run_test_specs = []

            # Check for instances that have already been run
            for test_spec in test_specs:
                log_dir = get_log_dir(
                    predictions[test_spec.instance_id], run_id, test_spec.instance_id
                )
                if log_dir.exists():
                    continue
                run_test_specs.append(test_spec)

            if run_test_specs:
                # Run instances that haven't been run yet
                results = run_instance_modal.starmap(
                    [
                        (
                            test_spec,
                            predictions[test_spec.instance_id],
                            run_id,
                            timeout,
                        )
                        for test_spec in run_test_specs
                    ],
                    return_exceptions=True,
                )

                for result in results:
                    if not isinstance(result, TestOutput):
                        print(f"Result failed with error: {result}")
                        continue

                    # Save logs locally
                    log_dir = result.log_dir
                    log_dir.mkdir(parents=True, exist_ok=True)
                    with open(log_dir / "run_instance.log", "w") as f:
                        f.write(result.run_instance_log)
                    with open(log_dir / "test_output.txt", "w") as f:
                        f.write(result.test_output)
                    with open(log_dir / "patch.diff", "w") as f:
                        f.write(result.patch_diff)
                    with open(log_dir / "report.json", "w") as f:
                        try:
                            report_json = json.loads(result.report_json_str)
                            json.dump(report_json, f, indent=4)
                        except Exception:
                            # This happens if the test fails with any exception
                            print(f"{result.instance_id}: no report.json")

            make_run_report(predictions, full_dataset, run_id) |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            run_evaluation_modal_entrypoint

######
            STDIO_RATE_LIMIT_BYTES_PER_SEC

  `module-attribute`

```
STDIO_RATE_LIMIT_BYTES_PER_SEC = 64 * 1024 // 2
```

######
            parser

  `module-attribute`

```
parser = ArgumentParser(description='Execute a shell command and stream output')
```

######
            args

  `module-attribute`

```
args = parse_args()
```

######
            exec

  `async`

```
exec(command: str) -> int
```

Source code in `swebench/harness/modal_eval/run_evaluation_modal_entrypoint.py` | 16
 17
 18
 19
 20
 21
 22
 23
 24
 25
 26
 27
 28
 29
 30
 31
 32
 33
 34
 35
 36
 37
 38
 39
 40
 41
 42
 43
 44
 45
 46
 47
 48
 49
 50
 51
 52
 53
 54
 55
 56
 57
 58
 59
 60
 61
 62
 63
 64
 65
 66
 67
 68
 69
 70
 71
 72
 73
 74
 75
 76
 77
 78
 79
 80
 81
 82
 83
 84
 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108 | async def exec(command: str) -> int:
    p = await asyncio.create_subprocess_shell(
        command,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
        limit=1024 * 1024,
    )

    stdout_lines = []
    stderr_lines = []

    async def read_stream(stream, lines, fd):
        tokens = STDIO_RATE_LIMIT_BYTES_PER_SEC
        last_refill = asyncio.get_event_loop().time()

        while True:
            try:
                line = await stream.readline()
                if not line:
                    break
            except (asyncio.LimitOverrunError, ValueError):
                # buffer exceeded asyncio stream limit
                fallback_chunk_size = 8192
                line = await stream.read(fallback_chunk_size)
                if not line:
                    break

            remaining_data = line
            buffer = bytearray()

            while remaining_data:
                current_time = asyncio.get_event_loop().time()
                time_passed = current_time - last_refill

                tokens = min(
                    STDIO_RATE_LIMIT_BYTES_PER_SEC,
                    tokens + (time_passed * STDIO_RATE_LIMIT_BYTES_PER_SEC),
                )
                last_refill = current_time

                chunk_size = min(
                    len(remaining_data), STDIO_RATE_LIMIT_BYTES_PER_SEC, int(tokens)
                )

                if chunk_size == 0:
                    sleep_time = max(
                        0.01,
                        (0.01 * STDIO_RATE_LIMIT_BYTES_PER_SEC - tokens)
                        / STDIO_RATE_LIMIT_BYTES_PER_SEC,
                    )
                    await asyncio.sleep(sleep_time)
                    continue

                buffer.extend(remaining_data[:chunk_size])

                # Find last valid UTF-8 character boundary.
                # This is to avoid partial characters being written to
                # container stdout/stderr, which results in a very small
                # chance of errors of the form: "Error reading stream: 'utf-8' codec can't decode bytes in position ..."
                valid_bytes = len(
                    buffer.decode("utf-8", errors="ignore").encode("utf-8")
                )

                if valid_bytes > 0:
                    chunk = buffer[:valid_bytes]
                    if fd == "stdout":
                        sys.stdout.buffer.write(chunk)
                        sys.stdout.buffer.flush()
                    else:
                        sys.stderr.buffer.write(chunk)
                        sys.stderr.buffer.flush()

                    buffer = buffer[valid_bytes:]
                    tokens -= valid_bytes

                remaining_data = remaining_data[chunk_size:]

            if buffer:
                if fd == "stdout":
                    sys.stdout.buffer.write(buffer)
                    sys.stdout.buffer.flush()
                else:
                    sys.stderr.buffer.write(buffer)
                    sys.stderr.buffer.flush()

            lines.append(line)

    await asyncio.gather(
        read_stream(p.stdout, stdout_lines, "stdout"),
        read_stream(p.stderr, stderr_lines, "stderr"),
    )

    return await p.wait() |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            main

  `async`

```
main(command: str)
```

Source code in `swebench/harness/modal_eval/run_evaluation_modal_entrypoint.py` | 111
112
113 | async def main(command: str):
    returncode = await exec(command)
    exit(returncode) |
| ----------- | --------------------------------------------------------------------------------------- |

#####
            utils

######
            validate_modal_credentials

```
validate_modal_credentials()
```

Validate that Modal credentials exist by checking for ~/.modal.toml file.
Raises an exception if credentials are not configured.

Source code in `swebench/harness/modal_eval/utils.py` | 4
 5
 6
 7
 8
 9
10
11
12
13
14 | def validate_modal_credentials():
    """
    Validate that Modal credentials exist by checking for ~/.modal.toml file.
    Raises an exception if credentials are not configured.
    """
    modal_config_path = Path.home() / ".modal.toml"
    if not modal_config_path.exists():
        raise RuntimeError(
            "~/.modal.toml not found - it looks like you haven't configured credentials for Modal.\n"
            "Run 'modal token new' in your terminal to configure credentials."
        ) |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            prepare_images

#####
            parser

  `module-attribute`

```
parser = ArgumentParser()
```

#####
            args

  `module-attribute`

```
args = parse_args()
```

#####
            filter_dataset_to_build

```
filter_dataset_to_build(dataset: list, instance_ids: list | None, client: DockerClient, force_rebuild: bool, namespace: str = None, tag: str = None, env_image_tag: str = None)
```

Filter the dataset to only include instances that need to be built.

Parameters:

| Name          | Type         | Description                                                 | Default  |
| ------------- | ------------ | ----------------------------------------------------------- | -------- |
| dataset       | list         | List of instances (usually all of SWE-bench dev/test split) | required |
| instance_ids  | list         | List of instance IDs to build.                              | required |
| client        | DockerClient | Docker client.                                              | required |
| force_rebuild | bool         | Whether to force rebuild all images.                        | required |

Source code in `swebench/harness/prepare_images.py` | 13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62 | def filter_dataset_to_build(
    dataset: list,
    instance_ids: list \| None,
    client: docker.DockerClient,
    force_rebuild: bool,
    namespace: str = None,
    tag: str = None,
    env_image_tag: str = None,
):
    """
    Filter the dataset to only include instances that need to be built.

    Args:
        dataset (list): List of instances (usually all of SWE-bench dev/test split)
        instance_ids (list): List of instance IDs to build.
        client (docker.DockerClient): Docker client.
        force_rebuild (bool): Whether to force rebuild all images.
    """
    # Get existing images
    existing_images = list_images(client)
    data_to_build = []

    if instance_ids is None:
        instance_ids = [instance[KEY_INSTANCE_ID] for instance in dataset]

    # Check if all instance IDs are in the dataset
    not_in_dataset = set(instance_ids).difference(
        set([instance[KEY_INSTANCE_ID] for instance in dataset])
    )
    if not_in_dataset:
        raise ValueError(f"Instance IDs not found in dataset: {not_in_dataset}")

    for instance in dataset:
        if instance[KEY_INSTANCE_ID] not in instance_ids:
            # Skip instances not in the list
            continue

        # Check if the instance needs to be built (based on force_rebuild flag and existing images)
        spec = make_test_spec(
            instance,
            namespace=namespace,
            instance_image_tag=tag,
            env_image_tag=env_image_tag,
        )
        if force_rebuild:
            data_to_build.append(instance)
        elif spec.instance_image_key not in existing_images:
            data_to_build.append(instance)

    return data_to_build |
| ----------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            main

```
main(dataset_name, split, instance_ids, max_workers, force_rebuild, open_file_limit, namespace, tag, env_image_tag)
```

Build Docker images for the specified instances.

Parameters:

| Name            | Type | Description                                | Default  |
| --------------- | ---- | ------------------------------------------ | -------- |
| instance_ids    | list | List of instance IDs to build.             | required |
| max_workers     | int  | Number of workers for parallel processing. | required |
| force_rebuild   | bool | Whether to force rebuild all images.       | required |
| open_file_limit | int  | Open file limit.                           | required |

Source code in `swebench/harness/prepare_images.py` | 65
 66
 67
 68
 69
 70
 71
 72
 73
 74
 75
 76
 77
 78
 79
 80
 81
 82
 83
 84
 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110 | def main(
    dataset_name,
    split,
    instance_ids,
    max_workers,
    force_rebuild,
    open_file_limit,
    namespace,
    tag,
    env_image_tag,
):
    """
    Build Docker images for the specified instances.

    Args:
        instance_ids (list): List of instance IDs to build.
        max_workers (int): Number of workers for parallel processing.
        force_rebuild (bool): Whether to force rebuild all images.
        open_file_limit (int): Open file limit.
    """
    # Set open file limit
    resource.setrlimit(resource.RLIMIT_NOFILE, (open_file_limit, open_file_limit))
    client = docker.from_env()

    # Filter out instances that were not specified
    dataset = load_swebench_dataset(dataset_name, split)
    dataset = filter_dataset_to_build(
        dataset, instance_ids, client, force_rebuild, namespace, tag, env_image_tag
    )

    if len(dataset) == 0:
        print("All images exist. Nothing left to build.")
        return 0

    # Build images for remaining instances
    successful, failed = build_instance_images(
        client=client,
        dataset=dataset,
        force_rebuild=force_rebuild,
        max_workers=max_workers,
        namespace=namespace,
        tag=tag,
        env_image_tag=env_image_tag,
    )
    print(f"Successfully built {len(successful)} images")
    print(f"Failed to build {len(failed)} images") |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            remove_containers

#####
            parser

  `module-attribute`

```
parser = ArgumentParser(description=__doc__)
```

#####
            args

  `module-attribute`

```
args = parse_args()
```

#####
            instance_ids

  `module-attribute`

```
instance_ids = [(strip()) for i in (split(','))] if instance_ids else []
```

#####
            main

```
main(instance_ids, predictions_path)
```

Source code in `swebench/harness/remove_containers.py` | 11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37 | def main(instance_ids, predictions_path):
    all_ids = set()
    if predictions_path:
        with open(predictions_path, "r") as f:
            predictions = json.loads(f.read())
            for pred in predictions:
                all_ids.add(pred["instance_id"])

    if instance_ids:
        all_ids \|= set(instance_ids)

    if not all_ids:
        print("No instance IDs provided, exiting.")
        return

    for instance_id in all_ids:
        try:
            client = docker.from_env()
            container = client.containers.get(f"sweb.eval.{instance_id}")
            container.stop()
            container.remove()
            print(f"Removed container {instance_id}")
        except docker.errors.NotFound:
            print(f"Container {instance_id} not found, skipping.")
        except Exception as e:
            print(f"Error removing container {instance_id}: {e}")
            continue |
| -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            reporting

#####
            make_run_report

```
make_run_report(predictions: dict, full_dataset: list, run_id: str, client: Optional[DockerClient] = None, namespace: str = None, instance_image_tag: str = 'latest', env_image_tag: str = 'latest') -> Path
```

Make a final evaluation and run report of the instances that have been run.
Also reports on images and containers that may still running if client is provided.

Parameters:

| Name         | Type         | Description                             | Default  |
| ------------ | ------------ | --------------------------------------- | -------- |
| predictions  | dict         | Predictions dict generated by the model | required |
| full_dataset | list         | List of all instances                   | required |
| run_id       | str          | Run ID                                  | required |
| client       | DockerClient | Docker client (optional)                | None     |

Returns:

| Type | Description         |
| ---- | ------------------- |
| Path | Path to report file |

Source code in `swebench/harness/reporting.py` | 17
 18
 19
 20
 21
 22
 23
 24
 25
 26
 27
 28
 29
 30
 31
 32
 33
 34
 35
 36
 37
 38
 39
 40
 41
 42
 43
 44
 45
 46
 47
 48
 49
 50
 51
 52
 53
 54
 55
 56
 57
 58
 59
 60
 61
 62
 63
 64
 65
 66
 67
 68
 69
 70
 71
 72
 73
 74
 75
 76
 77
 78
 79
 80
 81
 82
 83
 84
 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159
160 | def make_run_report(
    predictions: dict,
    full_dataset: list,
    run_id: str,
    client: Optional[docker.DockerClient] = None,
    namespace: str = None,
    instance_image_tag: str = "latest",
    env_image_tag: str = "latest",
) -> Path:
    """
    Make a final evaluation and run report of the instances that have been run.
    Also reports on images and containers that may still running if client is provided.

    Args:
        predictions (dict): Predictions dict generated by the model
        full_dataset (list): List of all instances
        run_id (str): Run ID
        client (docker.DockerClient): Docker client (optional)

    Returns:
        Path to report file
    """
    # instantiate sets to store IDs of different outcomes
    completed_ids = set()
    resolved_ids = set()
    error_ids = set()
    unstopped_containers = set()
    unremoved_images = set()
    unresolved_ids = set()
    incomplete_ids = set()
    # get instances with empty patches
    empty_patch_ids = set()

    # iterate through dataset and check if the instance has been run
    for instance in full_dataset:
        instance_id = instance[KEY_INSTANCE_ID]
        if instance_id not in predictions:
            # skip instances without predictions
            incomplete_ids.add(instance_id)
            continue
        prediction = predictions[instance_id]
        if prediction.get(KEY_PREDICTION, None) in ["", None]:
            empty_patch_ids.add(instance_id)
            continue
        report_file = (
            RUN_EVALUATION_LOG_DIR
            / run_id
            / prediction[KEY_MODEL].replace("/", "__")
            / prediction[KEY_INSTANCE_ID]
            / LOG_REPORT
        )
        if report_file.exists():
            completed_ids.add(instance_id)
            try:
                content = report_file.read_text().strip()
                if not content:  # Empty file
                    error_ids.add(instance_id)
                    continue

                report = json.loads(content)
                if report[instance_id]["resolved"]:
                    # Record if the instance was resolved
                    resolved_ids.add(instance_id)
                else:
                    unresolved_ids.add(instance_id)
            except (json.JSONDecodeError, KeyError):
                # If the report file is not valid JSON or missing keys, treat as error
                error_ids.add(instance_id)
        else:
            # Otherwise, the instance was not run successfully
            error_ids.add(instance_id)

    if client:
        # get remaining images and containers
        images = list_images(client)
        test_specs = list(
            map(
                lambda x: make_test_spec(
                    x,
                    namespace=namespace,
                    instance_image_tag=instance_image_tag,
                    env_image_tag=env_image_tag,
                ),
                full_dataset,
            )
        )
        for spec in test_specs:
            image_name = spec.instance_image_key
            if image_name in images:
                unremoved_images.add(image_name)
        containers = client.containers.list(all=True)
        for container in containers:
            if run_id in container.name:
                unstopped_containers.add(container.name)

    # print final report
    dataset_ids = {i[KEY_INSTANCE_ID] for i in full_dataset}
    print(f"Total instances: {len(full_dataset)}")
    print(f"Instances submitted: {len(set(predictions.keys()) & dataset_ids)}")
    print(f"Instances completed: {len(completed_ids)}")
    print(f"Instances incomplete: {len(incomplete_ids)}")
    print(f"Instances resolved: {len(resolved_ids)}")
    print(f"Instances unresolved: {len(unresolved_ids)}")
    print(f"Instances with empty patches: {len(empty_patch_ids)}")
    print(f"Instances with errors: {len(error_ids)}")
    if client:
        print(f"Unstopped containers: {len(unstopped_containers)}")
        print(f"Unremoved images: {len(unremoved_images)}")

    # write report to file
    report = {
        "total_instances": len(full_dataset),
        "submitted_instances": len(predictions),
        "completed_instances": len(completed_ids),
        "resolved_instances": len(resolved_ids),
        "unresolved_instances": len(unresolved_ids),
        "empty_patch_instances": len(empty_patch_ids),
        "error_instances": len(error_ids),
        "completed_ids": list(sorted(completed_ids)),
        "incomplete_ids": list(sorted(incomplete_ids)),
        "empty_patch_ids": list(sorted(empty_patch_ids)),
        "submitted_ids": list(sorted(predictions.keys())),
        "resolved_ids": list(sorted(resolved_ids)),
        "unresolved_ids": list(sorted(unresolved_ids)),
        "error_ids": list(sorted(error_ids)),
        "schema_version": 2,
    }
    if not client:
        report.update(
            {
                "unstopped_instances": len(unstopped_containers),
                "unstopped_containers": list(sorted(unstopped_containers)),
                "unremoved_images": list(sorted(unremoved_images)),
            }
        )
    report_file = Path(
        list(predictions.values())[0][KEY_MODEL].replace("/", "__")
        + f".{run_id}"
        + ".json"
    )
    with open(report_file, "w") as f:
        print(json.dumps(report, indent=4), file=f)
    print(f"Report written to {report_file}")
    return report_file |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            run_evaluation

#####
            GIT_APPLY_CMDS

  `module-attribute`

```
GIT_APPLY_CMDS = ['git apply --verbose', 'git apply --verbose --reject', 'patch --batch --fuzz=5 -p1 -i']
```

#####
            parser

  `module-attribute`

```
parser = ArgumentParser(description='Run evaluation harness for the given dataset and predictions.', formatter_class=ArgumentDefaultsHelpFormatter)
```

#####
            args

  `module-attribute`

```
args = parse_args()
```

#####
            run_instance

```
run_instance(test_spec: TestSpec, pred: dict, rm_image: bool, force_rebuild: bool, client: DockerClient, run_id: str, timeout: int | None = None, rewrite_reports: bool = False) -> dict
```

Run a single instance with the given prediction.

Parameters:

| Name            | Type         | Description                                                | Default  |
| --------------- | ------------ | ---------------------------------------------------------- | -------- |
| test_spec       | TestSpec     | TestSpec instance                                          | required |
| pred            | dict         | Prediction w/ model_name_or_path, model_patch, instance_id | required |
| rm_image        | bool         | Whether to remove the image after running                  | required |
| force_rebuild   | bool         | Whether to force rebuild the image                         | required |
| client          | DockerClient | Docker client                                              | required |
| run_id          | str          | Run ID                                                     | required |
| timeout         | int          | Timeout for running tests                                  | None     |
| rewrite_reports | bool         | True if eval run is just to reformat existing report       | False    |

Source code in `swebench/harness/run_evaluation.py` | 71
 72
 73
 74
 75
 76
 77
 78
 79
 80
 81
 82
 83
 84
 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159
160
161
162
163
164
165
166
167
168
169
170
171
172
173
174
175
176
177
178
179
180
181
182
183
184
185
186
187
188
189
190
191
192
193
194
195
196
197
198
199
200
201
202
203
204
205
206
207
208
209
210
211
212
213
214
215
216
217
218
219
220
221
222
223
224
225
226
227
228
229
230
231
232
233
234
235
236
237
238
239
240
241
242
243
244
245
246
247
248
249
250
251
252
253
254
255
256
257
258
259
260
261
262
263
264
265
266
267
268
269
270
271
272
273 | def run_instance(
    test_spec: TestSpec,
    pred: dict,
    rm_image: bool,
    force_rebuild: bool,
    client: docker.DockerClient,
    run_id: str,
    timeout: int \| None = None,
    rewrite_reports: bool = False,
) -> dict:
    """
    Run a single instance with the given prediction.

    Args:
        test_spec (TestSpec): TestSpec instance
        pred (dict): Prediction w/ model_name_or_path, model_patch, instance_id
        rm_image (bool): Whether to remove the image after running
        force_rebuild (bool): Whether to force rebuild the image
        client (docker.DockerClient): Docker client
        run_id (str): Run ID
        timeout (int): Timeout for running tests
        rewrite_reports (bool): True if eval run is just to reformat existing report
    """
    # Set up logging directory
    instance_id = test_spec.instance_id
    model_name_or_path = pred.get(KEY_MODEL, "None").replace("/", "__")
    log_dir = RUN_EVALUATION_LOG_DIR / run_id / model_name_or_path / instance_id

    # Set up report file
    report_path = log_dir / LOG_REPORT
    if rewrite_reports:
        test_output_path = log_dir / LOG_TEST_OUTPUT
        if not test_output_path.exists():
            raise ValueError(f"Test output file {test_output_path} does not exist")
        report = get_eval_report(
            test_spec=test_spec,
            prediction=pred,
            test_log_path=test_output_path,
            include_tests_status=True,
        )
        # Write report to report.json
        with open(report_path, "w") as f:
            f.write(json.dumps(report, indent=4))
        return {
            "completed": True,
            "resolved": report[instance_id]["resolved"],
        }
    if report_path.exists():
        report = json.loads(report_path.read_text())
        return {
            "completed": True,
            "resolved": report[instance_id]["resolved"],
        }

    if not test_spec.is_remote_image:
        # Link the image build dir in the log dir
        build_dir = INSTANCE_IMAGE_BUILD_DIR / test_spec.instance_image_key.replace(
            ":", "__"
        )
        image_build_link = log_dir / "image_build_dir"
        if not image_build_link.exists():
            try:
                # link the image build dir in the log dir
                image_build_link.symlink_to(
                    build_dir.absolute(), target_is_directory=True
                )
            except:
                # some error, idk why
                pass

    # Set up logger
    log_dir.mkdir(parents=True, exist_ok=True)
    log_file = log_dir / LOG_INSTANCE
    logger = setup_logger(instance_id, log_file)

    # Run the instance
    container = None
    eval_completed = False
    report = {}
    try:
        # Build + start instance container (instance image should already be built)
        container = build_container(
            test_spec, client, run_id, logger, rm_image, force_rebuild
        )
        container.start()
        logger.info(f"Container for {instance_id} started: {container.id}")

        # Copy model prediction as patch file to container
        patch_file = Path(log_dir / "patch.diff")
        patch_file.write_text(pred[KEY_PREDICTION] or "")
        logger.info(
            f"Intermediate patch for {instance_id} written to {patch_file}, now applying to container..."
        )
        copy_to_container(container, patch_file, PurePosixPath(DOCKER_PATCH))

        # Attempt to apply patch to container (TODO: FIX THIS)
        applied_patch = False
        for git_apply_cmd in GIT_APPLY_CMDS:
            val = container.exec_run(
                f"{git_apply_cmd} {DOCKER_PATCH}",
                workdir=DOCKER_WORKDIR,
                user=DOCKER_USER,
            )
            if val.exit_code == 0:
                logger.info(f"{APPLY_PATCH_PASS}:\n{val.output.decode(UTF8)}")
                applied_patch = True
                break
            else:
                logger.info(f"Failed to apply patch to container: {git_apply_cmd}")
        if not applied_patch:
            logger.info(f"{APPLY_PATCH_FAIL}:\n{val.output.decode(UTF8)}")
            raise EvaluationError(
                instance_id,
                f"{APPLY_PATCH_FAIL}:\n{val.output.decode(UTF8)}",
                logger,
            )

        # Get git diff before running eval script
        git_diff_output_before = (
            container.exec_run(
                "git -c core.fileMode=false diff", workdir=DOCKER_WORKDIR
            )
            .output.decode(UTF8)
            .strip()
        )
        logger.info(f"Git diff before:\n{git_diff_output_before}")

        eval_file = Path(log_dir / "eval.sh")
        eval_file.write_text(test_spec.eval_script)
        logger.info(
            f"Eval script for {instance_id} written to {eval_file}; copying to container..."
        )
        copy_to_container(container, eval_file, PurePosixPath("/eval.sh"))

        # Run eval script, write output to logs
        test_output, timed_out, total_runtime = exec_run_with_timeout(
            container, "/bin/bash /eval.sh", timeout
        )
        test_output_path = log_dir / LOG_TEST_OUTPUT
        logger.info(f"Test runtime: {total_runtime:_.2f} seconds")
        with open(test_output_path, "w") as f:
            f.write(test_output)
            logger.info(f"Test output for {instance_id} written to {test_output_path}")
            if timed_out:
                f.write(f"\n\nTimeout error: {timeout} seconds exceeded.")
                raise EvaluationError(
                    instance_id,
                    f"Test timed out after {timeout} seconds.",
                    logger,
                )

        # Get git diff after running eval script (ignore permission changes)
        git_diff_output_after = (
            container.exec_run(
                "git -c core.fileMode=false diff", workdir=DOCKER_WORKDIR
            )
            .output.decode(UTF8)
            .strip()
        )

        # Check if git diff changed after running eval script
        logger.info(f"Git diff after:\n{git_diff_output_after}")
        if git_diff_output_after != git_diff_output_before:
            logger.info("Git diff changed after running eval script")

        # Get report from test output
        logger.info(f"Grading answer for {instance_id}...")
        report = get_eval_report(
            test_spec=test_spec,
            prediction=pred,
            test_log_path=test_output_path,
            include_tests_status=True,
        )
        logger.info(
            f"report: {report}\n"
            f"Result for {instance_id}: resolved: {report[instance_id]['resolved']}"
        )

        # Write report to report.json
        with open(report_path, "w") as f:
            f.write(json.dumps(report, indent=4))
        eval_completed = True
    except (EvaluationError, BuildImageError) as e:
        error_msg = traceback.format_exc()
        logger.info(error_msg)
        print(e)
    except Exception as e:
        error_msg = (
            f"Error in evaluating model for {instance_id}: {e}\n"
            f"{traceback.format_exc()}\n"
            f"Check ({logger.log_file}) for more information."
        )
        logger.error(error_msg)
    finally:
        # Remove instance container + image, close logger
        cleanup_container(client, container, logger)
        if rm_image:
            remove_image(client, test_spec.instance_image_key, logger)
        close_logger(logger)
        return {
            "completed": eval_completed,
            "resolved": report.get(instance_id, {}).get("resolved", False),
        } |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            run_instances

```
run_instances(predictions: dict, instances: list, cache_level: str, clean: bool, force_rebuild: bool, max_workers: int, run_id: str, timeout: int, namespace: str | None = 'swebench', instance_image_tag: str = 'latest', env_image_tag: str = 'latest', rewrite_reports: bool = False)
```

Run all instances for the given predictions in parallel.

Parameters:

| Name          | Type | Description                             | Default  |
| ------------- | ---- | --------------------------------------- | -------- |
| predictions   | dict | Predictions dict generated by the model | required |
| instances     | list | List of instances                       | required |
| cache_level   | str  | Cache level                             | required |
| clean         | bool | Clean images above cache level          | required |
| force_rebuild | bool | Force rebuild images                    | required |
| max_workers   | int  | Maximum number of workers               | required |
| run_id        | str  | Run ID                                  | required |
| timeout       | int  | Timeout for running tests               | required |

Source code in `swebench/harness/run_evaluation.py` | 276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292
293
294
295
296
297
298
299
300
301
302
303
304
305
306
307
308
309
310
311
312
313
314
315
316
317
318
319
320
321
322
323
324
325
326
327
328
329
330
331
332
333
334
335
336
337
338
339
340
341
342
343
344
345
346
347
348
349
350
351
352
353
354
355
356
357
358
359
360
361
362
363
364
365
366
367
368
369
370
371 | def run_instances(
    predictions: dict,
    instances: list,
    cache_level: str,
    clean: bool,
    force_rebuild: bool,
    max_workers: int,
    run_id: str,
    timeout: int,
    namespace: str \| None = "swebench",
    instance_image_tag: str = "latest",
    env_image_tag: str = "latest",
    rewrite_reports: bool = False,
):
    """
    Run all instances for the given predictions in parallel.

    Args:
        predictions (dict): Predictions dict generated by the model
        instances (list): List of instances
        cache_level (str): Cache level
        clean (bool): Clean images above cache level
        force_rebuild (bool): Force rebuild images
        max_workers (int): Maximum number of workers
        run_id (str): Run ID
        timeout (int): Timeout for running tests
    """
    client = docker.from_env()
    test_specs = list(
        map(
            lambda instance: make_test_spec(
                instance,
                namespace=namespace,
                instance_image_tag=instance_image_tag,
                env_image_tag=env_image_tag,
            ),
            instances,
        )
    )

    # print number of existing instance images
    instance_image_ids = {x.instance_image_key for x in test_specs}
    existing_images = {
        tag
        for i in client.images.list(all=True)
        for tag in i.tags
        if tag in instance_image_ids
    }
    if not force_rebuild and len(existing_images):
        print(
            f"Found {len(existing_images)} existing instance images. Will reuse them."
        )

    # run instances in parallel
    payloads = []
    for test_spec in test_specs:
        payloads.append(
            (
                test_spec,
                predictions[test_spec.instance_id],
                should_remove(
                    test_spec.instance_image_key,
                    cache_level,
                    clean,
                    existing_images,
                ),
                force_rebuild,
                client,
                run_id,
                timeout,
                rewrite_reports,
            )
        )

    # run instances in parallel
    print(f"Running {len(instances)} instances...")
    stats = {"✓": 0, "✖": 0, "error": 0}
    pbar = tqdm(total=len(payloads), desc="Evaluation", postfix=stats)
    lock = threading.Lock()

    def run_evaluation_with_progress(*args):
        result = run_instance(*args)
        with lock:
            if result["completed"]:
                if result["resolved"]:
                    stats["✓"] += 1
                else:
                    stats["✖"] += 1
            else:
                stats["error"] += 1
            pbar.set_postfix(stats)
            pbar.update()
        return result

    run_threadpool(run_evaluation_with_progress, payloads, max_workers)
    print("All instances run.") |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_dataset_from_preds

```
get_dataset_from_preds(dataset_name: str, split: str, instance_ids: list, predictions: dict, run_id: str, rewrite_reports: bool, exclude_completed: bool = True)
```

Return only instances that have predictions and are in the dataset.
If instance_ids is provided, only return instances with those IDs.
If exclude_completed is True, only return instances that have not been run yet.

Source code in `swebench/harness/run_evaluation.py` | 374
375
376
377
378
379
380
381
382
383
384
385
386
387
388
389
390
391
392
393
394
395
396
397
398
399
400
401
402
403
404
405
406
407
408
409
410
411
412
413
414
415
416
417
418
419
420
421
422
423
424
425
426
427
428
429
430
431
432
433
434
435
436
437
438
439
440
441
442
443
444
445
446
447
448
449
450
451
452
453
454
455
456
457
458
459
460
461
462
463
464
465
466
467
468
469
470
471 | def get_dataset_from_preds(
    dataset_name: str,
    split: str,
    instance_ids: list,
    predictions: dict,
    run_id: str,
    rewrite_reports: bool,
    exclude_completed: bool = True,
):
    """
    Return only instances that have predictions and are in the dataset.
    If instance_ids is provided, only return instances with those IDs.
    If exclude_completed is True, only return instances that have not been run yet.
    """
    # load dataset
    dataset = load_swebench_dataset(dataset_name, split)
    dataset_ids = {i[KEY_INSTANCE_ID] for i in dataset}

    if instance_ids:
        # check that all instance IDs have predictions
        missing_preds = set(instance_ids) - set(predictions.keys())
        if missing_preds:
            print(
                f"Warning: Missing predictions for {len(missing_preds)} instance IDs."
            )

    # check that all prediction IDs are in the dataset
    prediction_ids = set(predictions.keys())
    if prediction_ids - dataset_ids:
        raise ValueError(
            (
                "Some prediction IDs not found in dataset!"
                f"\nMissing IDs:\n{' '.join(prediction_ids - dataset_ids)}"
            )
        )
    if instance_ids:
        dataset = [i for i in dataset if i[KEY_INSTANCE_ID] in instance_ids]

    if rewrite_reports:
        # we only return instances that have existing test outputs
        test_output_ids = set()
        for instance in dataset:
            if instance[KEY_INSTANCE_ID] not in predictions:
                continue
            prediction = predictions[instance[KEY_INSTANCE_ID]]
            test_output_file = (
                RUN_EVALUATION_LOG_DIR
                / run_id
                / prediction["model_name_or_path"].replace("/", "__")
                / prediction[KEY_INSTANCE_ID]
                / "test_output.txt"
            )
            if test_output_file.exists():
                test_output_ids.add(instance[KEY_INSTANCE_ID])
        dataset = [
            i
            for i in dataset
            if i[KEY_INSTANCE_ID] in prediction_ids
            and i[KEY_INSTANCE_ID] in test_output_ids
        ]
        return dataset

    # check which instance IDs have already been run
    completed_ids = set()
    for instance in dataset:
        if instance[KEY_INSTANCE_ID] not in prediction_ids:
            # skip instances without predictions
            continue
        prediction = predictions[instance[KEY_INSTANCE_ID]]
        report_file = (
            RUN_EVALUATION_LOG_DIR
            / run_id
            / prediction[KEY_MODEL].replace("/", "__")
            / prediction[KEY_INSTANCE_ID]
            / LOG_REPORT
        )
        if report_file.exists():
            completed_ids.add(instance[KEY_INSTANCE_ID])

    if completed_ids and exclude_completed:
        # filter dataset to only instances that have not been run
        print(f"{len(completed_ids)} instances already run, skipping...")
        dataset = [i for i in dataset if i[KEY_INSTANCE_ID] not in completed_ids]

    empty_patch_ids = {
        k
        for k, v in predictions.items()
        if v[KEY_PREDICTION] == "" or v[KEY_PREDICTION] is None
    }

    # filter dataset to only instances with predictions
    dataset = [
        i
        for i in dataset
        if i[KEY_INSTANCE_ID] in prediction_ids
        and i[KEY_INSTANCE_ID] not in empty_patch_ids
    ]
    return dataset |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            main

```
main(dataset_name: str, split: str, instance_ids: list, predictions_path: str, max_workers: int, force_rebuild: bool, cache_level: str, clean: bool, open_file_limit: int, run_id: str, timeout: int, namespace: str | None, rewrite_reports: bool, modal: bool, instance_image_tag: str = 'latest', env_image_tag: str = 'latest', report_dir: str = '.')
```

Run evaluation harness for the given dataset and predictions.

Source code in `swebench/harness/run_evaluation.py` | 474
475
476
477
478
479
480
481
482
483
484
485
486
487
488
489
490
491
492
493
494
495
496
497
498
499
500
501
502
503
504
505
506
507
508
509
510
511
512
513
514
515
516
517
518
519
520
521
522
523
524
525
526
527
528
529
530
531
532
533
534
535
536
537
538
539
540
541
542
543
544
545
546
547
548
549
550
551
552
553
554
555
556
557
558
559
560
561
562
563
564
565
566
567
568
569
570
571
572
573
574
575
576
577 | def main(
    dataset_name: str,
    split: str,
    instance_ids: list,
    predictions_path: str,
    max_workers: int,
    force_rebuild: bool,
    cache_level: str,
    clean: bool,
    open_file_limit: int,
    run_id: str,
    timeout: int,
    namespace: str \| None,
    rewrite_reports: bool,
    modal: bool,
    instance_image_tag: str = "latest",
    env_image_tag: str = "latest",
    report_dir: str = ".",
):
    """
    Run evaluation harness for the given dataset and predictions.
    """
    if dataset_name == "SWE-bench/SWE-bench_Multimodal" and split == "test":
        print(
            "⚠️ Local evaluation for the test split of SWE-bench Multimodal is not supported. "
            "Please check out sb-cli (https://github.com/swe-bench/sb-cli/) for instructions on how to submit predictions."
        )
        return

    # set open file limit
    assert len(run_id) > 0, "Run ID must be provided"
    if report_dir is not None:
        report_dir = Path(report_dir)
        if not report_dir.exists():
            report_dir.mkdir(parents=True)

    if force_rebuild and namespace is not None:
        raise ValueError("Cannot force rebuild and use a namespace at the same time.")

    # load predictions as map of instance_id to prediction
    predictions = get_predictions_from_file(predictions_path, dataset_name, split)
    predictions = {pred[KEY_INSTANCE_ID]: pred for pred in predictions}

    # get dataset from predictions
    dataset = get_dataset_from_preds(
        dataset_name, split, instance_ids, predictions, run_id, rewrite_reports
    )
    full_dataset = load_swebench_dataset(dataset_name, split, instance_ids)

    if modal:
        # run instances on Modal
        if not dataset:
            print("No instances to run.")
        else:
            validate_modal_credentials()
            run_instances_modal(predictions, dataset, full_dataset, run_id, timeout)
        return

    # run instances locally
    if platform.system() == "Linux":
        resource.setrlimit(resource.RLIMIT_NOFILE, (open_file_limit, open_file_limit))
    client = docker.from_env()

    existing_images = list_images(client)
    if not dataset:
        print("No instances to run.")
    else:
        # build environment images + run instances
        if namespace is None and not rewrite_reports:
            build_env_images(
                client,
                dataset,
                force_rebuild,
                max_workers,
                namespace,
                instance_image_tag,
                env_image_tag,
            )
        run_instances(
            predictions,
            dataset,
            cache_level,
            clean,
            force_rebuild,
            max_workers,
            run_id,
            timeout,
            namespace=namespace,
            instance_image_tag=instance_image_tag,
            env_image_tag=env_image_tag,
            rewrite_reports=rewrite_reports,
        )

    # clean images + make final report
    clean_images(client, existing_images, cache_level, clean)
    return make_run_report(
        predictions,
        full_dataset,
        run_id,
        client,
        namespace,
        instance_image_tag,
        env_image_tag,
    ) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            test_spec

#####
            __all__

  `module-attribute`

```
__all__ = ['test_spec', 'create_scripts', 'javascript', 'python']
```

#####
            create_scripts

######
            make_repo_script_list

```
make_repo_script_list(specs, repo, repo_directory, base_commit, env_name) -> list
```

Create a list of bash commands to set up the repository for testing.
This is the setup script for the instance image.

Source code in `swebench/harness/test_spec/create_scripts.py` | 17
18
19
20
21
22
23
24
25
26 | def make_repo_script_list(specs, repo, repo_directory, base_commit, env_name) -> list:
    """
    Create a list of bash commands to set up the repository for testing.
    This is the setup script for the instance image.
    """
    ext = MAP_REPO_TO_EXT[repo]
    func = {
        "py": make_repo_script_list_py,
    }.get(ext, make_repo_script_list_common)
    return func(specs, repo, repo_directory, base_commit, env_name) |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            make_env_script_list

```
make_env_script_list(instance, specs, env_name) -> list
```

Creates the list of commands to set up the environment for testing.
This is the setup script for the environment image.

Source code in `swebench/harness/test_spec/create_scripts.py` | 29
30
31
32
33
34
35
36
37
38 | def make_env_script_list(instance, specs, env_name) -> list:
    """
    Creates the list of commands to set up the environment for testing.
    This is the setup script for the environment image.
    """
    ext = MAP_REPO_TO_EXT[instance["repo"]]
    func = {
        "py": make_env_script_list_py,
    }.get(ext, make_env_script_list_common)
    return func(instance, specs, env_name) |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_eval_script_list

```
make_eval_script_list(instance, specs, env_name, repo_directory, base_commit, test_patch) -> list
```

Applies the test patch and runs the tests.

Source code in `swebench/harness/test_spec/create_scripts.py` | 41
42
43
44
45
46
47
48
49
50
51
52
53 | def make_eval_script_list(
    instance, specs, env_name, repo_directory, base_commit, test_patch
) -> list:
    """
    Applies the test patch and runs the tests.
    """
    ext = MAP_REPO_TO_EXT[instance["repo"]]
    common_func = make_eval_script_list_common
    func = {
        "js": make_eval_script_list_js,
        "py": make_eval_script_list_py,
    }.get(ext, common_func)
    return func(instance, specs, env_name, repo_directory, base_commit, test_patch) |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            javascript

######
            MAP_REPO_TO_TEST_CMDS

  `module-attribute`

```
MAP_REPO_TO_TEST_CMDS = {'Automattic/wp-calypso': get_test_cmds_calypso}
```

######
            get_test_cmds_calypso

```
get_test_cmds_calypso(instance) -> list
```

Source code in `swebench/harness/test_spec/javascript.py` | 14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62 | def get_test_cmds_calypso(instance) -> list:
    test_paths = [x.path for x in PatchSet(instance["test_patch"])]
    test_cmds = []
    for test_path in test_paths:
        if re.search(r"__snapshots__/(.*).js.snap$", test_path):
            # Jest snapshots are not run directly
            test_path = "/".join(test_path.split("/")[:-2])

        # Determine which testing script to use
        if any([test_path.startswith(x) for x in ["client", "packages"]]):
            pkg = test_path.split("/")[0]
            if instance["version"] in [
                "10.10.0",
                "10.12.0",
                "10.13.0",
                "10.14.0",
                "10.15.2",
                "10.16.3",
            ]:
                test_cmds.append(
                    f"./node_modules/.bin/jest --verbose -c=test/{pkg}/jest.config.js '{test_path}'"
                )
            elif instance["version"] in [
                "6.11.5",
                "8.9.1",
                "8.9.3",
                "8.9.4",
                "8.11.0",
                "8.11.2",
                "10.4.1",
                "10.5.0",
                "10.6.0",
                "10.9.0",
            ]:
                test_cmds.append(
                    f"./node_modules/.bin/jest --verbose -c=test/{pkg}/jest.config.json '{test_path}'"
                )
            else:
                test_cmds.append(f"npm run test-{pkg} --verbose '{test_path}'")
        elif any([test_path.startswith(x) for x in ["test/e2e"]]):
            test_cmds.extend(
                [
                    "cd test/e2e",
                    f"NODE_CONFIG_ENV=test npm run test {test_path}",
                    "cd ../..",
                ]
            )

    return test_cmds |
| -------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            get_download_img_commands

```
get_download_img_commands(instance) -> list
```

Source code in `swebench/harness/test_spec/javascript.py` | 71
72
73
74
75
76
77
78
79
80
81
82
83
84 | def get_download_img_commands(instance) -> list:
    cmds = []
    image_assets = {}
    if "image_assets" in instance:
        if isinstance(instance["image_assets"], str):
            image_assets = json.loads(instance["image_assets"])
        else:
            image_assets = instance["image_assets"]
    for i in image_assets.get("test_patch", []):
        folder = Path(i["path"]).parent
        cmds.append(f"mkdir -p {folder}")
        cmds.append(f"curl -o {i['path']} {i['url']}")
        cmds.append(f"chmod 777 {i['path']}")
    return cmds |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_eval_script_list_js

```
make_eval_script_list_js(instance, specs, env_name, repo_directory, base_commit, test_patch) -> list
```

Applies the test patch and runs the tests.

Source code in `swebench/harness/test_spec/javascript.py` | 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105 | def make_eval_script_list_js(
    instance, specs, env_name, repo_directory, base_commit, test_patch
) -> list:
    """
    Applies the test patch and runs the tests.
    """
    eval_commands = make_eval_script_list_common(
        instance, specs, env_name, repo_directory, base_commit, test_patch
    )
    # Insert downloading right after reset command
    eval_commands[4:4] = get_download_img_commands(instance)
    if instance["repo"] in MAP_REPO_TO_TEST_CMDS:
        # Update test commands if they are custom commands
        test_commands = MAP_REPO_TO_TEST_CMDS[instance["repo"]](instance)
        idx_start_test_out = eval_commands.index(f": '{START_TEST_OUTPUT}'")
        idx_end_test_out = eval_commands.index(f": '{END_TEST_OUTPUT}'")
        eval_commands[idx_start_test_out + 1 : idx_end_test_out] = test_commands
    return eval_commands |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            python

######
            HEADERS

  `module-attribute`

```
HEADERS = {'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_11_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/50.0.2661.102 Safari/537.36'}
```

######
            REPLACE_REQ_PACKAGES

  `module-attribute`

```
REPLACE_REQ_PACKAGES = [('types-pkg_resources', 'types-setuptools')]
```

######
            get_environment_yml_by_commit

  `cached`

```
get_environment_yml_by_commit(repo: str, commit: str, env_name: str) -> str
```

Source code in `swebench/harness/test_spec/python.py` | 31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52 | @cache
def get_environment_yml_by_commit(repo: str, commit: str, env_name: str) -> str:
    for req_path in MAP_REPO_TO_ENV_YML_PATHS[repo]:
        reqs_url = posixpath.join(SWE_BENCH_URL_RAW, repo, commit, req_path)
        reqs = requests.get(reqs_url, headers=HEADERS)
        if reqs.status_code == 200:
            break
    else:
        raise ValueError(
            f"Could not find environment.yml at paths {MAP_REPO_TO_ENV_YML_PATHS[repo]} for repo {repo} at commit {commit}"
        )

    lines = reqs.text.split("\n")
    cleaned = []
    for line in lines:
        # Rename environment to given name
        if line.startswith("name:"):
            cleaned.append(f"name: {env_name}")
            continue
        cleaned.append(line)

    return "\n".join(cleaned) |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            clean_environment_yml

```
clean_environment_yml(yml_text: str) -> str
```

Clean environment.yml by removing packages that have been yanked from PyPI

conda style yamls take the form:
...
- channels:
    ...
- dependencies:
    ...
- pip:
    - pkg_to_replace
    - pkg_to_replace
- ... (more dependencies)

We want to replace packages in the pip section only.

Source code in `swebench/harness/test_spec/python.py` | 55
 56
 57
 58
 59
 60
 61
 62
 63
 64
 65
 66
 67
 68
 69
 70
 71
 72
 73
 74
 75
 76
 77
 78
 79
 80
 81
 82
 83
 84
 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108
109
110
111
112
113 | def clean_environment_yml(yml_text: str) -> str:
    """
    Clean environment.yml by removing packages that have been yanked from PyPI

    conda style yamls take the form:
    ...
    - channels:
        ...
    - dependencies:
        ...
    - pip:
        - pkg_to_replace
        - pkg_to_replace
    - ... (more dependencies)

    We want to replace packages in the pip section only.
    """
    pip_match = re.search(r"^(\s*-\s*pip\s*:\s*\n)", yml_text, flags=re.MULTILINE)
    if not pip_match:
        return yml_text
    pip_line_start = pip_match.start()
    # get indentation level of pip line
    pip_indent = len(pip_match.group(1)) - len(pip_match.group(1).lstrip())
    pip_content_start = pip_match.end()
    # find where pip section ends by looking for a line that's at same or less indentation
    # or a line that starts a new top-level dependency (not pip)
    lines_after_pip = yml_text[pip_content_start:].split("\n")
    pip_section_end = pip_content_start
    for ix, line in enumerate(lines_after_pip):
        if line.strip() == "":
            continue
        line_indent = len(line) - len(line.lstrip())
        if line_indent <= pip_indent:
            # +1 to account for the newline
            pip_section_end = pip_content_start + sum(
                len(l) + 1 for l in lines_after_pip[:ix]
            )
            break
    else:
        pip_section_end = len(yml_text)
    prefix = yml_text[:pip_content_start]
    pip_portion = yml_text[pip_content_start:pip_section_end]
    suffix = yml_text[pip_section_end:]
    for pkg_to_replace, replacement in REPLACE_REQ_PACKAGES:
        if replacement == None:
            pip_portion = re.sub(
                rf"^(\s*-\s*){re.escape(pkg_to_replace)}([<>~]=?.*\|$)\n?",
                "",
                pip_portion,
                flags=re.MULTILINE,
            )
        else:
            pip_portion = re.sub(
                rf"^(\s*-\s*){re.escape(pkg_to_replace)}([<>=!~]=?.*\|$)",
                rf"\1{replacement}",
                pip_portion,
                flags=re.MULTILINE,
            )
    return prefix + pip_portion + suffix |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            get_environment_yml

```
get_environment_yml(instance: SWEbenchInstance, env_name: str) -> str
```

Get environment.yml for given task instance

Parameters:

| Name     | Type | Description                                   | Default  |
| -------- | ---- | --------------------------------------------- | -------- |
| instance | dict | SWE Bench Task instance                       | required |
| env_name | str  | Rename retrieved environment.yml to this name | required |

Returns:
    environment.yml (str): Returns environment.yml as string

Source code in `swebench/harness/test_spec/python.py` | 116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134 | def get_environment_yml(instance: SWEbenchInstance, env_name: str) -> str:
    """
    Get environment.yml for given task instance

    Args:
        instance (dict): SWE Bench Task instance
        env_name (str): Rename retrieved environment.yml to this name
    Returns:
        environment.yml (str): Returns environment.yml as string
    """
    # Attempt to find environment.yml at each path based on task instance's repo
    commit = (
        instance["environment_setup_commit"]
        if "environment_setup_commit" in instance
        else instance["base_commit"]
    )
    yml_text = get_environment_yml_by_commit(instance["repo"], commit, env_name)
    yml_text = clean_environment_yml(yml_text)
    return yml_text |
| --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            get_requirements_by_commit

  `cached`

```
get_requirements_by_commit(repo: str, commit: str) -> str
```

Source code in `swebench/harness/test_spec/python.py` | 137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159
160
161
162
163
164
165
166
167
168
169
170
171
172
173
174
175
176
177
178
179
180
181 | @cache
def get_requirements_by_commit(repo: str, commit: str) -> str:
    for req_path in MAP_REPO_TO_REQS_PATHS[repo]:
        reqs_url = posixpath.join(SWE_BENCH_URL_RAW, repo, commit, req_path)
        reqs = requests.get(reqs_url, headers=HEADERS)
        if reqs.status_code == 200:
            break
    else:
        raise ValueError(
            f"Could not find requirements.txt at paths {MAP_REPO_TO_REQS_PATHS[repo]} for repo {repo} at commit {commit}"
        )

    lines = reqs.text
    original_req = []
    additional_reqs = []
    req_dir = "/".join(req_path.split("/")[:-1])
    exclude_line = lambda line: any(
        [line.strip().startswith(x) for x in ["-e .", "#", ".[test"]]
    )

    for line in lines.split("\n"):
        if line.strip().startswith("-r"):
            # Handle recursive requirements
            file_name = line[len("-r") :].strip()
            reqs_url = os.path.join(
                SWE_BENCH_URL_RAW,
                repo,
                commit,
                req_dir,
                file_name,
            )
            reqs = requests.get(reqs_url, headers=HEADERS)
            if reqs.status_code == 200:
                for line_extra in reqs.text.split("\n"):
                    if not exclude_line(line_extra):
                        additional_reqs.append(line_extra)
        else:
            if not exclude_line(line):
                original_req.append(line)

    # Combine all requirements into single text body
    additional_reqs.append("\n".join(original_req))
    all_reqs = "\n".join(additional_reqs)

    return all_reqs |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            clean_requirements

```
clean_requirements(requirements_text: str) -> str
```

Clean requirements.txt by replacing / removing packages

E.g. types-pkg_resources has been yanked from PyPI, so we replace it with types-setuptools

Source code in `swebench/harness/test_spec/python.py` | 184
185
186
187
188
189
190
191
192
193
194
195
196
197
198
199
200
201
202
203
204
205
206 | def clean_requirements(requirements_text: str) -> str:
    """
    Clean requirements.txt by replacing / removing packages

    E.g. types-pkg_resources has been yanked from PyPI, so we replace it with types-setuptools
    """
    for pkg_to_replace, replacement in REPLACE_REQ_PACKAGES:
        if replacement == None:
            requirements_text = re.sub(
                rf"^{re.escape(pkg_to_replace)}([<>=!~]=?.*\|$)\n?",
                "",
                requirements_text,
                flags=re.MULTILINE,
            )
        else:
            # this replacement removes version specifier of the original package
            requirements_text = re.sub(
                rf"^{re.escape(pkg_to_replace)}([<>=!~]=?.*\|$)",
                replacement,
                requirements_text,
                flags=re.MULTILINE,
            )
    return requirements_text |
| ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            get_requirements

```
get_requirements(instance: SWEbenchInstance) -> str
```

Get requirements.txt for given task instance

Parameters:

| Name     | Type | Description   | Default  |
| -------- | ---- | ------------- | -------- |
| instance | dict | task instance | required |

Returns:
    requirements.txt (str): Returns requirements.txt as string

Source code in `swebench/harness/test_spec/python.py` | 209
210
211
212
213
214
215
216
217
218
219
220
221
222
223
224
225
226
227 | def get_requirements(instance: SWEbenchInstance) -> str:
    """
    Get requirements.txt for given task instance

    Args:
        instance (dict): task instance
    Returns:
        requirements.txt (str): Returns requirements.txt as string
    """
    # Attempt to find requirements.txt at each path based on task instance's repo
    commit = (
        instance["environment_setup_commit"]
        if "environment_setup_commit" in instance
        else instance["base_commit"]
    )

    requirements_text = get_requirements_by_commit(instance["repo"], commit)
    requirements_text = clean_requirements(requirements_text)
    return requirements_text |
| --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            get_test_directives

```
get_test_directives(instance: SWEbenchInstance) -> list
```

Get test directives from the test_patch of a task instance

Parameters:

| Name     | Type | Description   | Default  |
| -------- | ---- | ------------- | -------- |
| instance | dict | task instance | required |

Returns:
    directives (list): List of test directives

Source code in `swebench/harness/test_spec/python.py` | 230
231
232
233
234
235
236
237
238
239
240
241
242
243
244
245
246
247
248
249
250
251
252
253
254
255
256
257
258
259
260
261 | def get_test_directives(instance: SWEbenchInstance) -> list:
    """
    Get test directives from the test_patch of a task instance

    Args:
        instance (dict): task instance
    Returns:
        directives (list): List of test directives
    """
    # For seq2seq code repos, testing command is fixed
    if instance["repo"] == "swe-bench/humaneval":
        return ["test.py"]

    # Get test directives from test patch and remove non-test files
    diff_pat = r"diff --git a/.* b/(.*)"
    test_patch = instance["test_patch"]
    directives = re.findall(diff_pat, test_patch)
    directives = [
        d for d in directives if not any(d.endswith(ext) for ext in NON_TEST_EXTS)
    ]

    # For Django tests, remove extension + "tests/" prefix and convert slashes to dots (module referencing)
    if instance["repo"] == "django/django":
        directives_transformed = []
        for d in directives:
            d = d[: -len(".py")] if d.endswith(".py") else d
            d = d[len("tests/") :] if d.startswith("tests/") else d
            d = d.replace("/", ".")
            directives_transformed.append(d)
        directives = directives_transformed

    return directives |
| ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            make_repo_script_list_py

```
make_repo_script_list_py(specs, repo, repo_directory, base_commit, env_name) -> list
```

Create a list of bash commands to set up the repository for testing.
This is the setup script for the instance image.

Source code in `swebench/harness/test_spec/python.py` | 264
265
266
267
268
269
270
271
272
273
274
275
276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292
293
294
295
296
297
298
299
300
301
302
303
304
305
306
307
308
309
310
311
312
313
314
315
316
317 | def make_repo_script_list_py(
    specs, repo, repo_directory, base_commit, env_name
) -> list:
    """
    Create a list of bash commands to set up the repository for testing.
    This is the setup script for the instance image.
    """
    branch = REPO_BASE_COMMIT_BRANCH.get(repo, {}).get(base_commit, "")
    branch = f"--branch {branch}" if branch else ""
    setup_commands = [
        f"git clone -o origin {branch} --single-branch https://github.com/{repo} {repo_directory}",
        f"chmod -R 777 {repo_directory}",  # So nonroot user can run tests
        f"cd {repo_directory}",
        f"git reset --hard {base_commit}",
        # Remove the remote and tags so the agent won't see newer commits.
        "git remote remove origin",
        # Remove only tags pointing to commits after target timestamp
        f"TARGET_TIMESTAMP=$(git show -s --format=%ci {base_commit})",
        'git tag -l \| while read tag; do TAG_COMMIT=$(git rev-list -n 1 "$tag"); TAG_TIME=$(git show -s --format=%ci "$TAG_COMMIT"); if [[ "$TAG_TIME" > "$TARGET_TIMESTAMP" ]]; then git tag -d "$tag"; fi; done',
        "git reflog expire --expire=now --all",
        "git gc --prune=now --aggressive",
        # Verify future logs aren't available
        "AFTER_TIMESTAMP=$(date -d \"$TARGET_TIMESTAMP + 1 second\" '+%Y-%m-%d %H:%M:%S')",
        'COMMIT_COUNT=$(git log --oneline --all --since="$AFTER_TIMESTAMP" \| wc -l)',
        '[ "$COMMIT_COUNT" -eq 0 ] \|\| exit 1',
        # Make sure conda is available for later use
        "source /opt/miniconda3/bin/activate",
        f"conda activate {env_name}",
        'echo "Current environment: $CONDA_DEFAULT_ENV"',
    ]
    if repo in MAP_REPO_TO_INSTALL:
        setup_commands.append(MAP_REPO_TO_INSTALL[repo])

    # Run pre-install set up if provided
    if "pre_install" in specs:
        for pre_install in specs["pre_install"]:
            setup_commands.append(pre_install)

    if "install" in specs:
        setup_commands.append(specs["install"])

    # If the setup modifies the repository in any way, it can be
    # difficult to get a clean diff.  This ensures that `git diff`
    # will only reflect the changes from the user while retaining the
    # original state of the repository plus setup commands.
    clean_diff_commands = [
        "git config --global user.email [email protected]",
        "git config --global user.name SWE-bench",
        "git commit --allow-empty -am SWE-bench",
    ]

    setup_commands += clean_diff_commands

    return setup_commands |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

######
            make_env_script_list_py_from_conda

```
make_env_script_list_py_from_conda(instance, specs, env_name, cached_environment_yml) -> list
```

Source code in `swebench/harness/test_spec/python.py` | 320
321
322
323
324
325
326
327
328
329
330 | def make_env_script_list_py_from_conda(
    instance, specs, env_name, cached_environment_yml
) -> list:
    HEREDOC_DELIMITER = "EOF_59812759871"
    reqs_commands = [
        "source /opt/miniconda3/bin/activate",
        f"cat <<'{HEREDOC_DELIMITER}' > /root/environment.yml\n{cached_environment_yml}\n{HEREDOC_DELIMITER}",
        "conda env create -f /root/environment.yml",
        f"conda activate {env_name}",
    ]
    return reqs_commands |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_env_script_list_py

```
make_env_script_list_py(instance, specs, env_name) -> list
```

Creates the list of commands to set up the conda environment for testing.
This is the setup script for the environment image.

Source code in `swebench/harness/test_spec/python.py` | 333
334
335
336
337
338
339
340
341
342
343
344
345
346
347
348
349
350
351
352
353
354
355
356
357
358
359
360
361
362
363
364
365
366
367
368
369
370
371
372
373
374
375
376
377
378
379
380
381
382
383
384
385
386
387
388
389
390
391
392
393
394
395
396
397
398
399
400
401
402 | def make_env_script_list_py(instance, specs, env_name) -> list:
    """
    Creates the list of commands to set up the conda environment for testing.
    This is the setup script for the environment image.
    """
    cached_environment_yml = load_cached_environment_yml(instance["instance_id"])
    if cached_environment_yml:
        return make_env_script_list_py_from_conda(
            instance, specs, env_name, cached_environment_yml
        )
    HEREDOC_DELIMITER = "EOF_59812759871"
    reqs_commands = [
        "source /opt/miniconda3/bin/activate",
    ]
    # Create conda environment according to install instructinos
    pkgs = specs.get("packages", "")
    if pkgs == "requirements.txt":
        # Create environment
        cmd = f"conda create -n {env_name} python={specs['python']} -y"
        reqs_commands.append(cmd)

        # Install dependencies
        reqs = get_requirements(instance)
        path_to_reqs = "$HOME/requirements.txt"
        reqs_commands.append(
            f"cat <<'{HEREDOC_DELIMITER}' > {path_to_reqs}\n{reqs}\n{HEREDOC_DELIMITER}"
        )
        cmd = f"conda activate {env_name} && python -m pip install -r {path_to_reqs}"
        reqs_commands.append(cmd)
        reqs_commands.append(f"rm {path_to_reqs}")
    elif pkgs == "environment.yml":
        # Create environment from yml
        reqs = get_environment_yml(instance, env_name)
        path_to_reqs = "environment.yml"
        reqs_commands.append(
            f"cat <<'{HEREDOC_DELIMITER}' > {path_to_reqs}\n{reqs}\n{HEREDOC_DELIMITER}"
        )
        if "no_use_env" in specs and specs["no_use_env"]:
            # `conda create` based installation
            cmd = (
                f"conda create -c conda-forge -n {env_name} python={specs['python']} -y"
            )
            reqs_commands.append(cmd)

            # Install dependencies
            cmd = f"conda env update -f {path_to_reqs}"
            reqs_commands.append(cmd)
        else:
            # `conda env create` based installation
            cmd = f"conda env create --file {path_to_reqs}"
            reqs_commands.append(cmd)

            cmd = f"conda activate {env_name} && conda install python={specs['python']} -y"
            reqs_commands.append(cmd)

        # Remove environment.yml
        reqs_commands.append(f"rm {path_to_reqs}")
    else:
        # Create environment + install dependencies
        cmd = f"conda create -n {env_name} python={specs['python']} {pkgs} -y"
        reqs_commands.append(cmd)

    reqs_commands.append(f"conda activate {env_name}")

    # Install additional packages if specified
    if "pip_packages" in specs:
        pip_packages = " ".join(specs["pip_packages"])
        cmd = f"python -m pip install {pip_packages}"
        reqs_commands.append(cmd)
    return reqs_commands |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_eval_script_list_py

```
make_eval_script_list_py(instance, specs, env_name, repo_directory, base_commit, test_patch) -> list
```

Applies the test patch and runs the tests.

Source code in `swebench/harness/test_spec/python.py` | 405
406
407
408
409
410
411
412
413
414
415
416
417
418
419
420
421
422
423
424
425
426
427
428
429
430
431
432
433
434
435
436
437
438
439
440
441
442
443
444
445
446
447
448
449
450
451
452
453
454
455
456
457
458
459
460
461
462 | def make_eval_script_list_py(
    instance, specs, env_name, repo_directory, base_commit, test_patch
) -> list:
    """
    Applies the test patch and runs the tests.
    """
    HEREDOC_DELIMITER = "EOF_114329324912"
    # Separate modified files (exist at base commit) from new files.
    # get_modified_files() only returns files with a real source (not /dev/null),
    # i.e. modified files. New files need `rm -f` instead of `git checkout`.
    # Without this, `git checkout {base_commit}` with no file args resets the
    # entire working tree, undoing image setup changes. (#518)
    modified_files = get_modified_files(test_patch)
    new_files = get_new_files(test_patch)
    reset_commands = []
    if modified_files:
        reset_commands.append(f"git checkout {base_commit} {' '.join(modified_files)}")
    if new_files:
        reset_commands.append(f"rm -f {' '.join(new_files)}")
    apply_test_patch_command = (
        f"git apply -v - <<'{HEREDOC_DELIMITER}'\n{test_patch}\n{HEREDOC_DELIMITER}"
    )
    test_command = " ".join(
        [
            MAP_REPO_VERSION_TO_SPECS[instance["repo"]][instance["version"]][
                "test_cmd"
            ],
            *get_test_directives(instance),
        ]
    )
    eval_commands = [
        "source /opt/miniconda3/bin/activate",
        f"conda activate {env_name}",
        f"cd {repo_directory}",
    ]
    if "eval_commands" in specs:
        eval_commands += specs["eval_commands"]
    eval_commands += [
        f"git config --global --add safe.directory {repo_directory}",  # for nonroot user
        f"cd {repo_directory}",
        # This is just informational, so we have a record
        "git status",
        "git show",
        f"git -c core.fileMode=false diff {base_commit}",
        "source /opt/miniconda3/bin/activate",
        f"conda activate {env_name}",
    ]
    if "install" in specs:
        eval_commands.append(specs["install"])
    eval_commands += reset_commands
    eval_commands += [
        apply_test_patch_command,
        f": '{START_TEST_OUTPUT}'",
        test_command,
        f": '{END_TEST_OUTPUT}'",
    ]
    eval_commands += reset_commands  # Revert tests after done
    return eval_commands |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            test_spec

######
            TestSpec

  `dataclass`

```
TestSpec(instance_id: str, repo: str, version: str, repo_script_list: list[str], eval_script_list: list[str], env_script_list: list[str], arch: str, FAIL_TO_PASS: list[str], PASS_TO_PASS: list[str], language: str, docker_specs: dict, namespace: Optional[str], base_image_tag: str = LATEST, env_image_tag: str = LATEST, instance_image_tag: str = LATEST)
```

A dataclass that represents a test specification for a single instance of SWE-bench.

instance_id `instance-attribute` ```
instance_id: str
```

repo `instance-attribute` ```
repo: str
```

version `instance-attribute` ```
version: str
```

repo_script_list `instance-attribute` ```
repo_script_list: list[str]
```

eval_script_list `instance-attribute` ```
eval_script_list: list[str]
```

env_script_list `instance-attribute` ```
env_script_list: list[str]
```

arch `instance-attribute` ```
arch: str
```

FAIL_TO_PASS `instance-attribute` ```
FAIL_TO_PASS: list[str]
```

PASS_TO_PASS `instance-attribute` ```
PASS_TO_PASS: list[str]
```

language `instance-attribute` ```
language: str
```

docker_specs `instance-attribute` ```
docker_specs: dict
```

namespace `instance-attribute` ```
namespace: Optional[str]
```

base_image_tag `class-attribute` `instance-attribute` ```
base_image_tag: str = LATEST
```

env_image_tag `class-attribute` `instance-attribute` ```
env_image_tag: str = LATEST
```

instance_image_tag `class-attribute` `instance-attribute` ```
instance_image_tag: str = LATEST
```

setup_env_script `property` ```
setup_env_script
```

eval_script `property` ```
eval_script
```

install_repo_script `property` ```
install_repo_script
```

base_image_key `property` ```
base_image_key
```

If docker_specs are present, the base image key includes a hash of the specs.

env_image_key `property` ```
env_image_key
```

The key for the environment image is based on the hash of the environment script list.
If the environment script list changes, the image will be rebuilt automatically.

Note that old images are not automatically deleted, so consider cleaning up old images periodically.

instance_image_key `property` ```
instance_image_key
```

is_remote_image `property` ```
is_remote_image
```

base_dockerfile `property` ```
base_dockerfile
```

env_dockerfile `property` ```
env_dockerfile
```

instance_dockerfile `property` ```
instance_dockerfile
```

platform `property` ```
platform
```

get_instance_container_name ```
get_instance_container_name(run_id=None)
```

Source code in `swebench/harness/test_spec/test_spec.py` | 117
118
119
120 | def get_instance_container_name(self, run_id=None):
    if not run_id:
        return f"sweb.eval.{self.instance_id}"
    return f"sweb.eval.{self.instance_id.lower()}.{run_id}" |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            get_test_specs_from_dataset

```
get_test_specs_from_dataset(dataset: Union[list[SWEbenchInstance], list[TestSpec]], namespace: Optional[str] = None, instance_image_tag: str = LATEST, env_image_tag: str = LATEST) -> list[TestSpec]
```

Idempotent function that converts a list of SWEbenchInstance objects to a list of TestSpec objects.

Source code in `swebench/harness/test_spec/test_spec.py` | 155
156
157
158
159
160
161
162
163
164
165
166
167
168
169
170
171 | def get_test_specs_from_dataset(
    dataset: Union[list[SWEbenchInstance], list[TestSpec]],
    namespace: Optional[str] = None,
    instance_image_tag: str = LATEST,
    env_image_tag: str = LATEST,
) -> list[TestSpec]:
    """
    Idempotent function that converts a list of SWEbenchInstance objects to a list of TestSpec objects.
    """
    if isinstance(dataset[0], TestSpec):
        return cast(list[TestSpec], dataset)
    return list(
        map(
            lambda x: make_test_spec(x, namespace, instance_image_tag, env_image_tag),
            cast(list[SWEbenchInstance], dataset),
        )
    ) |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_test_spec

```
make_test_spec(instance: SWEbenchInstance, namespace: Optional[str] = None, base_image_tag: str = LATEST, env_image_tag: str = LATEST, instance_image_tag: str = LATEST, arch: str = 'x86_64') -> TestSpec
```

Source code in `swebench/harness/test_spec/test_spec.py` | 174
175
176
177
178
179
180
181
182
183
184
185
186
187
188
189
190
191
192
193
194
195
196
197
198
199
200
201
202
203
204
205
206
207
208
209
210
211
212
213
214
215
216
217
218
219
220
221
222
223
224
225
226
227
228
229
230
231
232
233
234
235 | def make_test_spec(
    instance: SWEbenchInstance,
    namespace: Optional[str] = None,
    base_image_tag: str = LATEST,
    env_image_tag: str = LATEST,
    instance_image_tag: str = LATEST,
    arch: str = "x86_64",
) -> TestSpec:
    if isinstance(instance, TestSpec):
        return instance
    assert base_image_tag is not None, "base_image_tag cannot be None"
    assert env_image_tag is not None, "env_image_tag cannot be None"
    assert instance_image_tag is not None, "instance_image_tag cannot be None"
    instance_id = instance[KEY_INSTANCE_ID]
    repo = instance["repo"]
    version = instance.get("version")
    base_commit = instance["base_commit"]
    problem_statement = instance.get("problem_statement")
    hints_text = instance.get("hints_text")  # Unused
    test_patch = instance["test_patch"]

    def _from_json_or_obj(key: str) -> Any:
        """If key points to string, load with json"""
        if key not in instance:
            # If P2P, F2P keys not found, it's a validation instance
            return []
        if isinstance(instance[key], str):
            return json.loads(instance[key])
        return instance[key]

    pass_to_pass = _from_json_or_obj("PASS_TO_PASS")
    fail_to_pass = _from_json_or_obj("FAIL_TO_PASS")

    env_name = "testbed"
    repo_directory = f"/{env_name}"
    specs = MAP_REPO_VERSION_TO_SPECS[repo][version]
    docker_specs = specs.get("docker_specs", {})

    repo_script_list = make_repo_script_list(
        specs, repo, repo_directory, base_commit, env_name
    )
    env_script_list = make_env_script_list(instance, specs, env_name)
    eval_script_list = make_eval_script_list(
        instance, specs, env_name, repo_directory, base_commit, test_patch
    )
    return TestSpec(
        instance_id=instance_id,
        repo=repo,
        env_script_list=env_script_list,
        repo_script_list=repo_script_list,
        eval_script_list=eval_script_list,
        version=version,
        arch=arch,
        FAIL_TO_PASS=fail_to_pass,
        PASS_TO_PASS=pass_to_pass,
        language=MAP_REPO_TO_EXT[repo],
        docker_specs=docker_specs,
        namespace=namespace,
        base_image_tag=base_image_tag,
        env_image_tag=env_image_tag,
        instance_image_tag=instance_image_tag,
    ) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

#####
            utils

######
            get_test_cmds

```
get_test_cmds(instance) -> list
```

Source code in `swebench/harness/test_spec/utils.py` | 12
13
14
15
16 | def get_test_cmds(instance) -> list:
    test_cmd = MAP_REPO_VERSION_TO_SPECS[instance["repo"]][instance["version"]][
        "test_cmd"
    ]
    return [test_cmd] if isinstance(test_cmd, str) else test_cmd |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_repo_script_list_common

```
make_repo_script_list_common(specs, repo, repo_directory, base_commit, env_name) -> list
```

Create a list of bash commands to set up the repository for testing.
This is the setup script for the instance image.

Source code in `swebench/harness/test_spec/utils.py` | 22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39
40
41
42 | def make_repo_script_list_common(
    specs, repo, repo_directory, base_commit, env_name
) -> list:
    """
    Create a list of bash commands to set up the repository for testing.
    This is the setup script for the instance image.
    """
    setup_commands = [
        f"git clone -o origin https://github.com/{repo} {repo_directory}",
        f"chmod -R 777 {repo_directory}",  # So nonroot user can run tests
        f"cd {repo_directory}",
        f"git reset --hard {base_commit}",
        "git remote remove origin",  # Remove the remote so the agent won't see newer commits
    ]
    if "pre_install" in specs:
        setup_commands.extend(specs["pre_install"])
    if "install" in specs:
        setup_commands.extend(specs["install"])
    if "build" in specs:
        setup_commands.extend(specs["build"])
    return setup_commands |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_env_script_list_common

```
make_env_script_list_common(instance, specs, env_name) -> list
```

Creates the list of commands to set up the environment for testing.
This is the setup script for the environment image.

Source code in `swebench/harness/test_spec/utils.py` | 45
46
47
48
49
50
51
52
53
54
55
56 | def make_env_script_list_common(instance, specs, env_name) -> list:
    """
    Creates the list of commands to set up the environment for testing.
    This is the setup script for the environment image.
    """
    reqs_commands = []
    if "apt-pkgs" in specs:
        reqs_commands += [
            "apt-get update",
            f"apt-get install -y {' '.join(specs['apt-pkgs'])}",
        ]
    return reqs_commands |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            make_eval_script_list_common

```
make_eval_script_list_common(instance, specs, env_name, repo_directory, base_commit, test_patch) -> list
```

Applies the test patch and runs the tests.

Source code in `swebench/harness/test_spec/utils.py` | 59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
80
81
82
83
84
85
86
87
88
89
90
91
92
93
94
95 | def make_eval_script_list_common(
    instance, specs, env_name, repo_directory, base_commit, test_patch
) -> list:
    """
    Applies the test patch and runs the tests.
    """
    HEREDOC_DELIMITER = "EOF_114329324912"
    test_files = get_modified_files(test_patch)
    # Reset test files to the state they should be in before the patch.
    if test_files:
        reset_tests_command = f"git checkout {base_commit} {' '.join(test_files)}"
    else:
        reset_tests_command = 'echo "No test files to reset"'

    build_commands = []
    if "build" in specs:
        build_commands.extend(specs["build"])

    apply_test_patch_command = f"git apply --verbose --reject - <<'{HEREDOC_DELIMITER}'\n{test_patch}\n{HEREDOC_DELIMITER}"
    test_commands = get_test_cmds(instance)
    eval_commands = [
        f"cd {repo_directory}",
        f"git config --global --add safe.directory {repo_directory}",  # for nonroot user
        f"cd {repo_directory}",
        # This is just informational, so we have a record
        # f"git status",
        # f"git show",
        # f"git -c core.fileMode=false diff {base_commit}",
        reset_tests_command,
        apply_test_patch_command,
        *build_commands,
        f": '{START_TEST_OUTPUT}'",
        *test_commands,
        f": '{END_TEST_OUTPUT}'",
        reset_tests_command,
    ]
    return eval_commands |
| -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

####
            utils

#####
            PATCH_PATTERN

  `module-attribute`

```
PATCH_PATTERN = compile('(?:diff[\\w\\_\\.\\ \\/\\-]+\\n)?\\-\\-\\-\\s+a\\/(?:.*?)\\n\\+\\+\\+\\s+b\\/(?:.*?)(?=diff\\ |\\-\\-\\-\\ a\\/|\\Z)', DOTALL)
```

#####
            PATCH_FILE_PATTERN

  `module-attribute`

```
PATCH_FILE_PATTERN = compile('\\-\\-\\-\\s+a\\/(?:.+)\\n\\+\\+\\+\\s+b\\/(?:.+)')
```

#####
            PATCH_HUNK_PATTERN

  `module-attribute`

```
PATCH_HUNK_PATTERN = compile('\\@\\@\\s+\\-(\\d+),(\\d+)\\s+\\+(\\d+),(\\d+)\\s+\\@\\@(.+?)(?=diff\\ |\\-\\-\\-\\ a\\/|\\@\\@\\ \\-|\\Z)', DOTALL)
```

#####
            EvaluationError

```
EvaluationError(instance_id, message, logger)
```

              Bases: `Exception`

Source code in `swebench/harness/utils.py` | 26
27
28
29
30 | def __init__(self, instance_id, message, logger):
    super().__init__(message)
    self.instance_id = instance_id
    self.log_file = logger.log_file
    self.logger = logger |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

######
            instance_id

  `instance-attribute`

```
instance_id = instance_id
```

######
            log_file

  `instance-attribute`

```
log_file = log_file
```

######
            logger

  `instance-attribute`

```
logger = logger
```

######
            __str__

```
__str__()
```

Source code in `swebench/harness/utils.py` | 32
33
34
35
36
37
38 | def __str__(self):
    log_msg = traceback.format_exc()
    self.logger.info(log_msg)
    return (
        f"{self.instance_id}: {super().__str__()}\n"
        f"Check ({self.log_file}) for more information."
    ) |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_predictions_from_file

```
get_predictions_from_file(predictions_path: str, dataset_name: str, split: str)
```

Source code in `swebench/harness/utils.py` | 41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74
75
76
77 | def get_predictions_from_file(predictions_path: str, dataset_name: str, split: str):
    if predictions_path == "gold":
        print("Using gold predictions")
        dataset = load_swebench_dataset(dataset_name, split)
        return [
            {
                KEY_INSTANCE_ID: datum[KEY_INSTANCE_ID],
                KEY_PREDICTION: datum["patch"],
                KEY_MODEL: "gold",
            }
            for datum in dataset
        ]
    if predictions_path.endswith(".json"):
        with open(predictions_path, "r") as f:
            predictions = json.load(f)
            if isinstance(predictions, dict):
                predictions = list(
                    predictions.values()
                )  # compatible with SWE-agent predictions
            if not isinstance(predictions, list):
                raise ValueError(
                    "Predictions must be a list[prediction] or a dictionary[instance_id: prediction]"
                )
    elif predictions_path.endswith(".jsonl"):
        with open(predictions_path, "r") as f:
            predictions = [json.loads(line) for line in f]
    else:
        raise ValueError("Predictions path must be .json or .jsonl")

    # Validate that each prediction has an instance_id
    for pred in predictions:
        if not isinstance(pred, dict):
            raise ValueError(f"Each prediction must be a dictionary, got {type(pred)}")
        if KEY_INSTANCE_ID not in pred:
            raise ValueError(f"Each prediction must contain '{KEY_INSTANCE_ID}'")

    return predictions |
| -------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            run_threadpool

```
run_threadpool(func, payloads, max_workers)
```

Run a function with a list of payloads using ThreadPoolExecutor.

Parameters:

| Name        | Type | Description                      | Default  |
| ----------- | ---- | -------------------------------- | -------- |
| func        |      | Function to run for each payload | required |
| payloads    |      | List of payloads to process      | required |
| max_workers |      | Maximum number of worker threads | required |

Returns:

| Name  | Type | Description                           |
| ----- | ---- | ------------------------------------- |
| tuple |      | (succeeded, failed) lists of payloads |

Source code in `swebench/harness/utils.py` | 80
 81
 82
 83
 84
 85
 86
 87
 88
 89
 90
 91
 92
 93
 94
 95
 96
 97
 98
 99
100
101
102
103
104
105
106
107
108 | def run_threadpool(func, payloads, max_workers):
    """
    Run a function with a list of payloads using ThreadPoolExecutor.

    Args:
        func: Function to run for each payload
        payloads: List of payloads to process
        max_workers: Maximum number of worker threads

    Returns:
        tuple: (succeeded, failed) lists of payloads
    """
    if max_workers <= 0:
        return run_sequential(func, payloads)
    succeeded, failed = [], []
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        # Create a future for running each instance
        futures = {executor.submit(func, *payload): payload for payload in payloads}
        # Wait for each future to complete
        for future in as_completed(futures):
            try:
                # Check if instance ran successfully
                future.result()
                succeeded.append(futures[future])
            except Exception as e:
                print(f"{type(e)}: {e}")
                traceback.print_exc()
                failed.append(futures[future])
    return succeeded, failed |
| ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            run_sequential

```
run_sequential(func, payloads)
```

Run a function with a list of payloads sequentially.

Parameters:

| Name     | Type | Description                      | Default  |
| -------- | ---- | -------------------------------- | -------- |
| func     |      | Function to run for each payload | required |
| payloads |      | List of payloads to process      | required |

Returns:

| Name  | Type | Description                           |
| ----- | ---- | ------------------------------------- |
| tuple |      | (succeeded, failed) lists of payloads |

Source code in `swebench/harness/utils.py` | 111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130 | def run_sequential(func, payloads):
    """
    Run a function with a list of payloads sequentially.

    Args:
        func: Function to run for each payload
        payloads: List of payloads to process

    Returns:
        tuple: (succeeded, failed) lists of payloads
    """
    succeeded, failed = [], []
    for payload in payloads:
        try:
            func(*payload)
            succeeded.append(payload)
        except Exception:
            traceback.print_exc()
            failed.append(payload)
    return succeeded, failed |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            load_swebench_dataset

```
load_swebench_dataset(name='SWE-bench/SWE-bench', split='test', instance_ids=None) -> list[SWEbenchInstance]
```

Load SWE-bench dataset from Hugging Face Datasets or local .json/.jsonl file

Source code in `swebench/harness/utils.py` | 133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159
160
161
162
163
164
165
166
167
168
169
170
171
172
173
174
175
176
177
178
179
180
181
182 | def load_swebench_dataset(
    name="SWE-bench/SWE-bench", split="test", instance_ids=None
) -> list[SWEbenchInstance]:
    """
    Load SWE-bench dataset from Hugging Face Datasets or local .json/.jsonl file
    """
    # check that all instance IDs are in the dataset
    if instance_ids:
        instance_ids = set(instance_ids)
    # Load from local .json/.jsonl file
    if name.endswith(".json"):
        dataset = json.loads(Path(name).read_text())
    elif name.endswith(".jsonl"):
        dataset = [json.loads(line) for line in Path(name).read_text().splitlines()]
    elif name.endswith(".parquet"):
        dataset = cast(Dataset, load_dataset("parquet", data_files=name, split="train"))
    else:
        # Load from Hugging Face Datasets
        if name.lower() in {"swe-bench", "swebench", "swe_bench"}:
            name = "SWE-bench/SWE-bench"
        elif name.lower() in {
            "swe-bench-lite",
            "swebench-lite",
            "swe_bench_lite",
            "swe-bench_lite",
            "lite",
        }:
            name = "SWE-bench/SWE-bench_Lite"
        parquet_path = Path(name) / f"{split}.parquet"
        if parquet_path.exists():
            dataset = cast(Dataset, load_dataset("parquet", data_files=str(parquet_path), split="train"))
        elif (Path(name) / split / "dataset_info.json").exists():
            dataset = cast(Dataset, load_from_disk(Path(name) / split))
        else:
            dataset = cast(Dataset, load_dataset(name, split=split))
    dataset_ids = {instance[KEY_INSTANCE_ID] for instance in dataset}
    if instance_ids:
        if instance_ids - dataset_ids:
            raise ValueError(
                (
                    "Some instance IDs not found in dataset!"
                    f"\nMissing IDs:\n{' '.join(instance_ids - dataset_ids)}"
                )
            )
        dataset = [
            instance
            for instance in dataset
            if instance[KEY_INSTANCE_ID] in instance_ids
        ]
    return [cast(SWEbenchInstance, instance) for instance in dataset] |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_first_idx

```
get_first_idx(charlist)
```

Get index of first occurrence of "-" or "+" in charlist

Source code in `swebench/harness/utils.py` | 197
198
199
200
201 | def get_first_idx(charlist):
    """Get index of first occurrence of "-" or "+" in charlist"""
    first_min = charlist.index("-") if "-" in charlist else len(charlist)
    first_plus = charlist.index("+") if "+" in charlist else len(charlist)
    return min(first_min, first_plus) |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_last_idx

```
get_last_idx(charlist)
```

Get index of last occurrence of "-" or "+" in charlist

Source code in `swebench/harness/utils.py` | 204
205
206
207
208 | def get_last_idx(charlist):
    """Get index of last occurrence of "-" or "+" in charlist"""
    char_idx = get_first_idx(charlist[::-1])
    last_idx = len(charlist) - char_idx
    return last_idx + 1 |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            strip_content

```
strip_content(hunk)
```

Remove trailing non +/- lines and trailing whitespace per line per hunk

Source code in `swebench/harness/utils.py` | 211
212
213
214
215
216
217
218
219
220 | def strip_content(hunk):
    """Remove trailing non +/- lines and trailing whitespace per line per hunk"""
    first_chars = list(map(lambda x: None if not len(x) else x[0], hunk.split("\n")))
    first_idx = get_first_idx(first_chars)
    last_idx = get_last_idx(first_chars)
    new_lines = list(map(lambda x: x.rstrip(), hunk.split("\n")[first_idx:last_idx]))
    # should leave one space for empty context lines
    new_lines = [line if line.strip() else " " for line in new_lines]
    new_hunk = "\n" + "\n".join(new_lines) + "\n"
    return new_hunk, first_idx - 1 |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

#####
            get_hunk_stats

```
get_hunk_stats(pre_start, pre_len, post_start, post_len, hunk, total_delta)
```

Recalculate hunk start/end position and diff delta

Source code in `swebench/harness/utils.py` | 223
224
225
226
227
228
229
230
231
232
233
234
235
236
237
238
239
240
241 | def get_hunk_stats(pre_start, pre_len, post_start, post_len, hunk, total_delta):
    """Recalculate hunk start/end position and diff delta"""
    stats = {"context": 0, "added": 0, "subtracted": 0}
    hunk = hunk.split("\n", 1)[-1].strip("\n")
    for line in hunk.split("\n"):
        if line.startswith("-"):
            stats["subtracted"] += 1
        elif line.startswith("+"):
            stats["added"] += 1
        else:
            stats["context"] += 1
    context = stats["context"]
    added = stats["added"]
    subtracted = stats["subtracted"]
    pre_len = context + subtracted
    post_start = pre_start + total_delta
    post_len = context + added
    total_delta = total_delta + (post_len - pre_len)
    return pre_start, pre_len, post_start, post_len, total_delta |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            extract_minimal_patch

```
extract_minimal_patch(model_patch)
```

Wrapper function that takes hunk and
* Removes trailing non +/- lines and trailing whitespace per line per hunk
* Recalculates hunk start/end position and diff delta
* Returns new patch

Source code in `swebench/harness/utils.py` | 244
245
246
247
248
249
250
251
252
253
254
255
256
257
258
259
260
261
262
263
264
265
266
267
268
269
270
271 | def extract_minimal_patch(model_patch):
    """
    Wrapper function that takes hunk and
    * Removes trailing non +/- lines and trailing whitespace per line per hunk
    * Recalculates hunk start/end position and diff delta
    * Returns new patch
    """
    model_patch = model_patch.lstrip("\n")
    new_patch = ""
    for patch in PATCH_PATTERN.findall(model_patch):
        total_delta = 0
        patch_header = PATCH_FILE_PATTERN.findall(patch)[0]
        if patch_header:
            new_patch += patch_header + "\n"
        for hunk in PATCH_HUNK_PATTERN.findall(patch):
            pre_start, pre_len, post_start, post_len, content = hunk
            pre_start, pre_len, post_start, post_len, content = list(
                map(lambda x: int(x) if x.isnumeric() else x, hunk)
            )
            content, adjust_pre_start = strip_content(content)
            pre_start += adjust_pre_start
            pre_start, pre_len, post_start, post_len, total_delta = get_hunk_stats(
                pre_start, pre_len, post_start, post_len, content, total_delta
            )
            new_patch += (
                f"@@ -{pre_start},{pre_len} +{post_start},{post_len} @@{content}"
            )
    return new_patch |
| --------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            has_attribute_or_import_error

```
has_attribute_or_import_error(log_before)
```

Check to see if Attribute/Import-prefix is in log text

Parameters:

| Name       | Type | Description                                  | Default  |
| ---------- | ---- | -------------------------------------------- | -------- |
| log_before | str  | Validation log text before patch application | required |

Source code in `swebench/harness/utils.py` | 274
275
276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292
293
294
295
296
297
298
299
300
301
302 | def has_attribute_or_import_error(log_before):
    """
    Check to see if Attribute/Import-prefix is in log text

    Args:
        log_before (str): Validation log text before patch application
    """
    log_before = log_before.lower()

    if any([x in log_before for x in ["attribute", "import"]]):

        def get_lines_with_word(text, target_word):
            # Function to extract line(s) that contains target_word
            text, target_word = text.lower(), target_word.lower()
            lines, hits = text.split("\n")[::-1], []
            for line in lines:
                if target_word in line:
                    hits.append(line)
            return hits

        # Get line with Attribute/Import error
        lines_1 = get_lines_with_word(log_before, "attribute")
        lines_2 = get_lines_with_word(log_before, "import")
        lines_1 = " ".join(lines_1)
        lines_2 = " ".join(lines_2)

        if any([(x in lines_1 or x in lines_2) for x in ["error", "fail"]]):
            return True
    return False |
| ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            str2bool

```
str2bool(v)
```

Minor helper function to convert string to boolean

Source code in `swebench/harness/utils.py` | 305
306
307
308
309
310
311
312
313
314
315
316 | def str2bool(v):
    """
    Minor helper function to convert string to boolean
    """
    if isinstance(v, bool):
        return v
    if v.lower() in ("yes", "true", "t", "y", "1"):
        return True
    elif v.lower() in ("no", "false", "f", "n", "0"):
        return False
    else:
        raise ArgumentTypeError("Boolean value expected.") |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |

#####
            optional_str

```
optional_str(value: str) -> str | None
```

Convert special string values to None, otherwise return the string as-is.

Source code in `swebench/harness/utils.py` | 319
320
321
322
323
324
325 | def optional_str(value: str) -> str \| None:
    """
    Convert special string values to None, otherwise return the string as-is.
    """
    if value.lower() in ("none", "null", ""):
        return None
    return value |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_repo_file

```
get_repo_file(repo, commit, filepath)
```

Source code in `swebench/harness/utils.py` | 328
329
330
331
332
333
334
335
336 | def get_repo_file(repo, commit, filepath):
    url = f"https://raw.githubusercontent.com/{repo}/{commit}/{filepath}"
    try:
        response = requests.get(url)
        if response.status_code == 200:
            return response.text
        return None
    except:
        return None |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_modified_files

```
get_modified_files(patch: str) -> list[str]
```

Get the list of modified files in a patch (excludes new files).

Source code in `swebench/harness/utils.py` | 339
340
341
342
343
344
345
346
347
348 | def get_modified_files(patch: str) -> list[str]:
    """
    Get the list of modified files in a patch (excludes new files).
    """
    source_files = []
    for file in PatchSet(patch):
        if file.source_file != "/dev/null":
            source_files.append(file.source_file)
    source_files = [x[2:] for x in source_files if x.startswith("a/")]
    return source_files |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            get_new_files

```
get_new_files(patch: str) -> list[str]
```

Get the list of new files in a patch (source is /dev/null).

Source code in `swebench/harness/utils.py` | 351
352
353
354
355
356
357
358
359
360
361
362 | def get_new_files(patch: str) -> list[str]:
    """
    Get the list of new files in a patch (source is /dev/null).
    """
    new_files = []
    for file in PatchSet(patch):
        if file.source_file == "/dev/null":
            target = file.target_file
            if target.startswith("b/"):
                target = target[2:]
            new_files.append(target)
    return new_files |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            ansi_escape

```
ansi_escape(text: str) -> str
```

Remove ANSI escape sequences from text

Source code in `swebench/harness/utils.py` | 365
366
367
368
369 | def ansi_escape(text: str) -> str:
    """
    Remove ANSI escape sequences from text
    """
    return re.compile(r"\x1B(?:[@-Z\\-_]\|\[[0-?]*[ -/]*[@-~])").sub("", text) |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

#####
            load_cached_environment_yml

```
load_cached_environment_yml(instance_id: str) -> str
```

Load environment.yml from cache

Source code in `swebench/harness/utils.py` | 372
373
374
375
376
377
378
379
380
381
382
383
384
385
386
387 | def load_cached_environment_yml(instance_id: str) -> str:
    """
    Load environment.yml from cache
    """
    try:
        repo, number = instance_id.rsplit("-", 1)
    except ValueError:
        return None
    try:
        return (
            resources.files(swebench.resources)
            .joinpath(f"swebench-og/{repo}/{number}/environment.yml")
            .read_text()
        )
    except FileNotFoundError:
        return None |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

[

                Previous

                Versioning

          ](https://www.swebench.com/SWE-bench/api/harness/../../reference/versioning/) [

                Next

                Inference

          ](https://www.swebench.com/SWE-bench/api/harness/../inference/)

    Made with
    [
      Material for MkDocs
    ](https://squidfunk.github.io/mkdocs-material/)
