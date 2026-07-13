# REST API endpoints for starring - GitHub Enterprise Server 3.12 Docs

Source: https://docs.github.com/en/enterprise-server@3.12/rest/activity/starring

[Skip to main content](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#main-content)[GitHub Docs](https://docs.github.com/en/enterprise-server@3.12)Version: Enterprise Server 3.12Search GitHub DocsSearchSelect language: current language is EnglishOpen Search BarClose Search BarOpen MenuOpen Sidebar- [REST API](https://docs.github.com/en/enterprise-server@3.12/rest)/
- [Activity](https://docs.github.com/en/enterprise-server@3.12/rest/activity)/
- [Starring](https://docs.github.com/en/enterprise-server@3.12/rest/activity/starring)

[Home](https://docs.github.com/en/enterprise-server@3.12)[REST API](https://docs.github.com/en/rest)API Version: 2022-11-28 (latest)- [Quickstart](https://docs.github.com/en/enterprise-server@3.12/rest/quickstart)
- About the REST API- [About the REST API](https://docs.github.com/en/enterprise-server@3.12/rest/about-the-rest-api/about-the-rest-api)
- [Comparing GitHub's APIs](https://docs.github.com/en/enterprise-server@3.12/rest/about-the-rest-api/comparing-githubs-rest-api-and-graphql-api)
- [API Versions](https://docs.github.com/en/enterprise-server@3.12/rest/about-the-rest-api/api-versions)
- [Breaking changes](https://docs.github.com/en/enterprise-server@3.12/rest/about-the-rest-api/breaking-changes)
- [OpenAPI description](https://docs.github.com/en/enterprise-server@3.12/rest/about-the-rest-api/about-the-openapi-description-for-the-rest-api)

- Using the REST API- [Getting started](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/getting-started-with-the-rest-api)
- [Rate limits](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/rate-limits-for-the-rest-api)
- [Pagination](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/using-pagination-in-the-rest-api)
- [Libraries](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/libraries-for-the-rest-api)
- [Best practices](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/best-practices-for-using-the-rest-api)
- [Troubleshooting](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/troubleshooting-the-rest-api)
- [Timezones](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/timezones-and-the-rest-api)
- [CORS and JSONP](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/using-cors-and-jsonp-to-make-cross-origin-requests)
- [Issue event types](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/issue-event-types)
- [GitHub event types](https://docs.github.com/en/enterprise-server@3.12/rest/using-the-rest-api/github-event-types)

- Authentication- [Authenticating](https://docs.github.com/en/enterprise-server@3.12/rest/authentication/authenticating-to-the-rest-api)
- [Keeping API credentials secure](https://docs.github.com/en/enterprise-server@3.12/rest/authentication/keeping-your-api-credentials-secure)
- [Endpoints for GitHub App installation tokens](https://docs.github.com/en/enterprise-server@3.12/rest/authentication/endpoints-available-for-github-app-installation-access-tokens)
- [Endpoints for GitHub App user tokens](https://docs.github.com/en/enterprise-server@3.12/rest/authentication/endpoints-available-for-github-app-user-access-tokens)
- [Endpoints for fine-grained PATs](https://docs.github.com/en/enterprise-server@3.12/rest/authentication/endpoints-available-for-fine-grained-personal-access-tokens)
- [Permissions for GitHub Apps](https://docs.github.com/en/enterprise-server@3.12/rest/authentication/permissions-required-for-github-apps)
- [Permissions for fine-grained PATs](https://docs.github.com/en/enterprise-server@3.12/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens)

- Guides- [Script with JavaScript](https://docs.github.com/en/enterprise-server@3.12/rest/guides/scripting-with-the-rest-api-and-javascript)
- [Script with Ruby](https://docs.github.com/en/enterprise-server@3.12/rest/guides/scripting-with-the-rest-api-and-ruby)
- [Discover resources for a user](https://docs.github.com/en/enterprise-server@3.12/rest/guides/discovering-resources-for-a-user)
- [Delivering deployments](https://docs.github.com/en/enterprise-server@3.12/rest/guides/delivering-deployments)
- [Rendering data as graphs](https://docs.github.com/en/enterprise-server@3.12/rest/guides/rendering-data-as-graphs)
- [Working with comments](https://docs.github.com/en/enterprise-server@3.12/rest/guides/working-with-comments)
- [Building a CI server](https://docs.github.com/en/enterprise-server@3.12/rest/guides/building-a-ci-server)
- [Get started - Git database](https://docs.github.com/en/enterprise-server@3.12/rest/guides/using-the-rest-api-to-interact-with-your-git-database)
- [Get started - Checks](https://docs.github.com/en/enterprise-server@3.12/rest/guides/using-the-rest-api-to-interact-with-checks)
- [Encrypt secrets](https://docs.github.com/en/enterprise-server@3.12/rest/guides/encrypting-secrets-for-the-rest-api)

---

- Actions- Artifacts
- Cache
- OIDC
- Permissions
- Secrets
- Self-hosted runner groups
- Self-hosted runners
- Variables
- Workflow jobs
- Workflow runs
- Workflows

- Activity- Events
- Feeds
- Notifications
- Starring- [About starring](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#about-starring)
- [List stargazers](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-stargazers)
- [List repositories starred by the authenticated user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-the-authenticated-user)
- [Check if a repository is starred by the authenticated user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#check-if-a-repository-is-starred-by-the-authenticated-user)
- [Star a repository for the authenticated user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#star-a-repository-for-the-authenticated-user)
- [Unstar a repository for the authenticated user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#unstar-a-repository-for-the-authenticated-user)
- [List repositories starred by a user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-a-user)

- Watching

- Announcement banners- Organization

- Apps- GitHub Apps
- Installations
- OAuth authorizations
- Webhooks

- Billing- Billing

- Branches- Branches
- Protected branches

- Checks- Check runs
- Check suites

- Code scanning- Code scanning

- Codes of conduct- Codes of conduct

- Collaborators- Collaborators
- Invitations

- Commits- Commits
- Commit comments
- Commit statuses

- Dependabot- Alerts
- Secrets

- Dependency graph- Dependency review
- Dependency submission
- Software bill of materials (SBOM)

- Deploy keys- Deploy keys

- Deployments- Deployment branch policies
- Deployments
- Environments
- Protection rules
- Deployment statuses

- Emojis- Emojis

- Enterprise administration- Admin stats
- Announcement
- Audit log
- Billing
- Security features for code
- Global webhooks
- LDAP
- License
- Manage GHES
- Management Console
- Organization pre-receive hooks
- Organizations
- Pre-receive environments
- Pre-receive hooks
- Repository pre-receive hooks
- SCIM
- Users

- Gists- Gists
- Comments

- Git database- Blobs
- Commits
- References
- Tags
- Trees

- Gitignore- Gitignore

- Issues- Assignees
- Comments
- Events
- Issues
- Labels
- Milestones
- Timeline

- Licenses- Licenses

- Markdown- Markdown

- Meta- Meta

- Metrics- Statistics

- Migrations- Organizations
- Users

- OAuth app authorizations- OAuth app authorizations

- Organizations- Custom roles
- Members
- Organizations
- Outside collaborators
- Personal access tokens
- Rule suites
- Rules
- Security managers
- Webhooks

- Packages- Packages

- Pages- Pages

- Projects (classic)- Boards
- Cards
- Collaborators
- Columns

- Pull requests- Pull requests
- Review comments
- Review requests
- Reviews

- Rate limit- Rate limit

- Reactions- Reactions

- Releases- Releases
- Release assets

- Repositories- Autolinks
- Contents
- Forks
- Git LFS
- Repositories
- Rule suites
- Rules
- Tags
- Webhooks

- Search- Search

- Secret scanning- Secret scanning

- Security advisories- Global security advisories

- Teams- Teams
- Discussion comments
- Discussions
- External groups
- Members

- Users- Emails
- Followers
- GPG keys
- Git SSH keys
- Social accounts
- SSH signing keys
- Users

**This version of GitHub Enterprise Server was discontinued on 2025-04-03.** No patch releases will be made, even for critical security issues. For better performance, improved security, and new features, [upgrade to the latest version of GitHub Enterprise Server](https://docs.github.com/admin/upgrading-your-instance/preparing-to-upgrade/overview-of-the-upgrade-process).
For help with the upgrade, [contact GitHub Enterprise support](https://enterprise.github.com/support).

The REST API is now versioned. For more information, see "[About API versioning](https://docs.github.com/enterprise-server@3.12/rest/overview/api-versions)."- [REST API](https://docs.github.com/en/enterprise-server@3.12/rest)/
- [Activity](https://docs.github.com/en/enterprise-server@3.12/rest/activity)/
- [Starring](https://docs.github.com/en/enterprise-server@3.12/rest/activity/starring)

# REST API endpoints for starring

Use the REST API to bookmark a repository.

## [About starring](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#about-starring)

You can use the REST API to star (bookmark) a repository. Stars are shown next to repositories to show an approximate level of interest. Stars have no effect on notifications or the activity feed. For more information, see [Saving repositories with stars](https://docs.github.com/en/enterprise-server@3.12/get-started/exploring-projects-on-github/saving-repositories-with-stars).

### [Starring versus watching](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#starring-versus-watching)

In August 2012, we [changed the way watching
works](https://github.com/blog/1204-notifications-stars) on GitHub. Some API
client applications may still be using the original "watcher" endpoints for accessing
this data. You should now use the "star" endpoints instead (described
below). For more information, see [REST API endpoints for watching](https://docs.github.com/en/enterprise-server@3.12/rest/activity/watching) and the [changelog post](https://developer.github.com/changes/2012-09-05-watcher-api/).

In responses from the REST API, `watchers`, `watchers_count`, and `stargazers_count` correspond to the number of users that have starred a repository, whereas `subscribers_count` corresponds to the number of watchers.

## [List stargazers](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-stargazers)

Lists the people that have starred the repository.

This endpoint supports the following custom media types. For more information, see "[Media types](https://docs.github.com/enterprise-server@3.12/rest/using-the-rest-api/getting-started-with-the-rest-api#media-types)."

- **`application/vnd.github.star+json`**: Includes a timestamp of when the star was created.

### [Fine-grained access tokens for "List stargazers"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-stargazers--fine-grained-access-tokens)

This endpoint works with the following fine-grained token types:

- [GitHub App user access tokens](https://docs.github.com/en/enterprise-server@3.12/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [GitHub App installation access tokens](https://docs.github.com/en/enterprise-server@3.12/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app)
- [Fine-grained personal access tokens](https://docs.github.com/en/enterprise-server@3.12/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-fine-grained-personal-access-token)

The fine-grained token must have the following permission set:

- "Metadata" repository permissions (read)

This endpoint can be used without authentication or the aforementioned permissions if only public resources are requested.

### [Parameters for "List stargazers"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-stargazers--parameters)

| Name, Type, Description                                              |
| -------------------------------------------------------------------- |
| accept string Setting to application/vnd.github+json is recommended. |

| Name, Type, Description                                                                                    |
| ---------------------------------------------------------------------------------------------------------- |
| owner string RequiredThe account owner of the repository. The name is not case sensitive.                  |
| repo string RequiredThe name of the repository without the .git extension. The name is not case sensitive. |

| Name, Type, Description                                                                                                             |
| ----------------------------------------------------------------------------------------------------------------------------------- |
| per_page integer The number of results per page (max 100). For more information, see "Using pagination in the REST API."Default: 30 |
| page integer The page number of the results to fetch. For more information, see "Using pagination in the REST API."Default: 1       |

### [HTTP response status codes for "List stargazers"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-stargazers--status-codes)

| Status code | Description                                          |
| ----------- | ---------------------------------------------------- |
| 200         | OK                                                   |
| 422         | Validation failed, or the endpoint has been spammed. |

### [Code samples for "List stargazers"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-stargazers--code-samples)

#### Request examples

Select the example typeExample 1: Status Code 200Example 2: Status Code 200get/repos/{owner}/{repo}/stargazers[cURL](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[JavaScript](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[GitHub CLI](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)Copy to clipboard curl request example`curl -L \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer <YOUR-TOKEN>" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  http(s)://HOSTNAME/api/v3/repos/OWNER/REPO/stargazers`#### Default response

[Example response](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[Response schema](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)`Status: 200``[
  {
    "login": "octocat",
    "id": 1,
    "node_id": "MDQ6VXNlcjE=",
    "avatar_url": "https://github.com/images/error/octocat_happy.gif",
    "gravatar_id": "",
    "url": "https://HOSTNAME/users/octocat",
    "html_url": "https://github.com/octocat",
    "followers_url": "https://HOSTNAME/users/octocat/followers",
    "following_url": "https://HOSTNAME/users/octocat/following{/other_user}",
    "gists_url": "https://HOSTNAME/users/octocat/gists{/gist_id}",
    "starred_url": "https://HOSTNAME/users/octocat/starred{/owner}{/repo}",
    "subscriptions_url": "https://HOSTNAME/users/octocat/subscriptions",
    "organizations_url": "https://HOSTNAME/users/octocat/orgs",
    "repos_url": "https://HOSTNAME/users/octocat/repos",
    "events_url": "https://HOSTNAME/users/octocat/events{/privacy}",
    "received_events_url": "https://HOSTNAME/users/octocat/received_events",
    "type": "User",
    "site_admin": false
  }
]`## [List repositories starred by the authenticated user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-the-authenticated-user)

Lists repositories the authenticated user has starred.

This endpoint supports the following custom media types. For more information, see "[Media types](https://docs.github.com/enterprise-server@3.12/rest/using-the-rest-api/getting-started-with-the-rest-api#media-types)."

- **`application/vnd.github.star+json`**: Includes a timestamp of when the star was created.

### [Fine-grained access tokens for "List repositories starred by the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-the-authenticated-user--fine-grained-access-tokens)

This endpoint works with the following fine-grained token types:

- [GitHub App user access tokens](https://docs.github.com/en/enterprise-server@3.12/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [Fine-grained personal access tokens](https://docs.github.com/en/enterprise-server@3.12/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-fine-grained-personal-access-token)

The fine-grained token must have the following permission set:

- "Starring" user permissions (read)

This endpoint can be used without authentication or the aforementioned permissions if only public resources are requested.

### [Parameters for "List repositories starred by the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-the-authenticated-user--parameters)

| Name, Type, Description                                              |
| -------------------------------------------------------------------- |
| accept string Setting to application/vnd.github+json is recommended. |

| Name, Type, Description                                                                                                                                                                              |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| sort string The property to sort the results by. created means when the repository was starred. updated means when the repository was last pushed to.Default: createdCan be one of: created, updated |
| direction string The direction to sort the results by.Default: descCan be one of: asc, desc                                                                                                          |
| per_page integer The number of results per page (max 100). For more information, see "Using pagination in the REST API."Default: 30                                                                  |
| page integer The page number of the results to fetch. For more information, see "Using pagination in the REST API."Default: 1                                                                        |

### [HTTP response status codes for "List repositories starred by the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-the-authenticated-user--status-codes)

| Status code | Description             |
| ----------- | ----------------------- |
| 200         | OK                      |
| 304         | Not modified            |
| 401         | Requires authentication |
| 403         | Forbidden               |

### [Code samples for "List repositories starred by the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-the-authenticated-user--code-samples)

#### Request examples

Select the example typeExample 1: Status Code 200 (application/json)Example 2: Status Code 200 (application/vnd.github.v3.star+json)get/user/starred[cURL](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[JavaScript](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[GitHub CLI](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)Copy to clipboard curl request example`curl -L \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer <YOUR-TOKEN>" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  http(s)://HOSTNAME/api/v3/user/starred`#### Default response

[Example response](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[Response schema](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)`Status: 200``[
  {
    "id": 1296269,
    "node_id": "MDEwOlJlcG9zaXRvcnkxMjk2MjY5",
    "name": "Hello-World",
    "full_name": "octocat/Hello-World",
    "owner": {
      "login": "octocat",
      "id": 1,
      "node_id": "MDQ6VXNlcjE=",
      "avatar_url": "https://github.com/images/error/octocat_happy.gif",
      "gravatar_id": "",
      "url": "https://HOSTNAME/users/octocat",
      "html_url": "https://github.com/octocat",
      "followers_url": "https://HOSTNAME/users/octocat/followers",
      "following_url": "https://HOSTNAME/users/octocat/following{/other_user}",
      "gists_url": "https://HOSTNAME/users/octocat/gists{/gist_id}",
      "starred_url": "https://HOSTNAME/users/octocat/starred{/owner}{/repo}",
      "subscriptions_url": "https://HOSTNAME/users/octocat/subscriptions",
      "organizations_url": "https://HOSTNAME/users/octocat/orgs",
      "repos_url": "https://HOSTNAME/users/octocat/repos",
      "events_url": "https://HOSTNAME/users/octocat/events{/privacy}",
      "received_events_url": "https://HOSTNAME/users/octocat/received_events",
      "type": "User",
      "site_admin": false
    },
    "private": false,
    "html_url": "https://github.com/octocat/Hello-World",
    "description": "This your first repo!",
    "fork": false,
    "url": "https://HOSTNAME/repos/octocat/Hello-World",
    "archive_url": "https://HOSTNAME/repos/octocat/Hello-World/{archive_format}{/ref}",
    "assignees_url": "https://HOSTNAME/repos/octocat/Hello-World/assignees{/user}",
    "blobs_url": "https://HOSTNAME/repos/octocat/Hello-World/git/blobs{/sha}",
    "branches_url": "https://HOSTNAME/repos/octocat/Hello-World/branches{/branch}",
    "collaborators_url": "https://HOSTNAME/repos/octocat/Hello-World/collaborators{/collaborator}",
    "comments_url": "https://HOSTNAME/repos/octocat/Hello-World/comments{/number}",
    "commits_url": "https://HOSTNAME/repos/octocat/Hello-World/commits{/sha}",
    "compare_url": "https://HOSTNAME/repos/octocat/Hello-World/compare/{base}...{head}",
    "contents_url": "https://HOSTNAME/repos/octocat/Hello-World/contents/{+path}",
    "contributors_url": "https://HOSTNAME/repos/octocat/Hello-World/contributors",
    "deployments_url": "https://HOSTNAME/repos/octocat/Hello-World/deployments",
    "downloads_url": "https://HOSTNAME/repos/octocat/Hello-World/downloads",
    "events_url": "https://HOSTNAME/repos/octocat/Hello-World/events",
    "forks_url": "https://HOSTNAME/repos/octocat/Hello-World/forks",
    "git_commits_url": "https://HOSTNAME/repos/octocat/Hello-World/git/commits{/sha}",
    "git_refs_url": "https://HOSTNAME/repos/octocat/Hello-World/git/refs{/sha}",
    "git_tags_url": "https://HOSTNAME/repos/octocat/Hello-World/git/tags{/sha}",
    "git_url": "git:github.com/octocat/Hello-World.git",
    "issue_comment_url": "https://HOSTNAME/repos/octocat/Hello-World/issues/comments{/number}",
    "issue_events_url": "https://HOSTNAME/repos/octocat/Hello-World/issues/events{/number}",
    "issues_url": "https://HOSTNAME/repos/octocat/Hello-World/issues{/number}",
    "keys_url": "https://HOSTNAME/repos/octocat/Hello-World/keys{/key_id}",
    "labels_url": "https://HOSTNAME/repos/octocat/Hello-World/labels{/name}",
    "languages_url": "https://HOSTNAME/repos/octocat/Hello-World/languages",
    "merges_url": "https://HOSTNAME/repos/octocat/Hello-World/merges",
    "milestones_url": "https://HOSTNAME/repos/octocat/Hello-World/milestones{/number}",
    "notifications_url": "https://HOSTNAME/repos/octocat/Hello-World/notifications{?since,all,participating}",
    "pulls_url": "https://HOSTNAME/repos/octocat/Hello-World/pulls{/number}",
    "releases_url": "https://HOSTNAME/repos/octocat/Hello-World/releases{/id}",
    "ssh_url": "git@github.com:octocat/Hello-World.git",
    "stargazers_url": "https://HOSTNAME/repos/octocat/Hello-World/stargazers",
    "statuses_url": "https://HOSTNAME/repos/octocat/Hello-World/statuses/{sha}",
    "subscribers_url": "https://HOSTNAME/repos/octocat/Hello-World/subscribers",
    "subscription_url": "https://HOSTNAME/repos/octocat/Hello-World/subscription",
    "tags_url": "https://HOSTNAME/repos/octocat/Hello-World/tags",
    "teams_url": "https://HOSTNAME/repos/octocat/Hello-World/teams",
    "trees_url": "https://HOSTNAME/repos/octocat/Hello-World/git/trees{/sha}",
    "clone_url": "https://github.com/octocat/Hello-World.git",
    "mirror_url": "git:git.example.com/octocat/Hello-World",
    "hooks_url": "https://HOSTNAME/repos/octocat/Hello-World/hooks",
    "svn_url": "https://svn.github.com/octocat/Hello-World",
    "homepage": "https://github.com",
    "language": null,
    "forks_count": 9,
    "stargazers_count": 80,
    "watchers_count": 80,
    "size": 108,
    "default_branch": "master",
    "open_issues_count": 0,
    "is_template": true,
    "topics": [
      "octocat",
      "atom",
      "electron",
      "api"
    ],
    "has_issues": true,
    "has_projects": true,
    "has_wiki": true,
    "has_pages": false,
    "has_downloads": true,
    "archived": false,
    "disabled": false,
    "visibility": "public",
    "pushed_at": "2011-01-26T19:06:43Z",
    "created_at": "2011-01-26T19:01:12Z",
    "updated_at": "2011-01-26T19:14:43Z",
    "permissions": {
      "admin": false,
      "push": false,
      "pull": true
    },
    "allow_rebase_merge": true,
    "template_repository": null,
    "temp_clone_token": "ABTLWHOULUVAXGTRYU7OC2876QJ2O",
    "allow_squash_merge": true,
    "allow_auto_merge": false,
    "delete_branch_on_merge": true,
    "allow_merge_commit": true,
    "subscribers_count": 42,
    "network_count": 0,
    "license": {
      "key": "mit",
      "name": "MIT License",
      "url": "https://HOSTNAME/licenses/mit",
      "spdx_id": "MIT",
      "node_id": "MDc6TGljZW5zZW1pdA==",
      "html_url": "https://github.com/licenses/mit"
    },
    "forks": 1,
    "open_issues": 1,
    "watchers": 1
  }
]`## [Check if a repository is starred by the authenticated user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#check-if-a-repository-is-starred-by-the-authenticated-user)

Whether the authenticated user has starred the repository.

### [Fine-grained access tokens for "Check if a repository is starred by the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#check-if-a-repository-is-starred-by-the-authenticated-user--fine-grained-access-tokens)

This endpoint works with the following fine-grained token types:

- [GitHub App user access tokens](https://docs.github.com/en/enterprise-server@3.12/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [Fine-grained personal access tokens](https://docs.github.com/en/enterprise-server@3.12/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-fine-grained-personal-access-token)

The fine-grained token must have the following permission set:

- "Metadata" repository permissions (read) and "Starring" user permissions (read)

### [Parameters for "Check if a repository is starred by the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#check-if-a-repository-is-starred-by-the-authenticated-user--parameters)

| Name, Type, Description                                              |
| -------------------------------------------------------------------- |
| accept string Setting to application/vnd.github+json is recommended. |

| Name, Type, Description                                                                                    |
| ---------------------------------------------------------------------------------------------------------- |
| owner string RequiredThe account owner of the repository. The name is not case sensitive.                  |
| repo string RequiredThe name of the repository without the .git extension. The name is not case sensitive. |

### [HTTP response status codes for "Check if a repository is starred by the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#check-if-a-repository-is-starred-by-the-authenticated-user--status-codes)

| Status code | Description                                        |
| ----------- | -------------------------------------------------- |
| 204         | Response if this repository is starred by you      |
| 304         | Not modified                                       |
| 401         | Requires authentication                            |
| 403         | Forbidden                                          |
| 404         | Not Found if this repository is not starred by you |

### [Code samples for "Check if a repository is starred by the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#check-if-a-repository-is-starred-by-the-authenticated-user--code-samples)

#### Request example

get/user/starred/{owner}/{repo}[cURL](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[JavaScript](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[GitHub CLI](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)Copy to clipboard curl request example`curl -L \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer <YOUR-TOKEN>" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  http(s)://HOSTNAME/api/v3/user/starred/OWNER/REPO`#### Response if this repository is starred by you

`Status: 204`## [Star a repository for the authenticated user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#star-a-repository-for-the-authenticated-user)

Note that you'll need to set `Content-Length` to zero when calling out to this endpoint. For more information, see "[HTTP method](https://docs.github.com/enterprise-server@3.12/rest/guides/getting-started-with-the-rest-api#http-method)."

### [Fine-grained access tokens for "Star a repository for the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#star-a-repository-for-the-authenticated-user--fine-grained-access-tokens)

This endpoint works with the following fine-grained token types:

- [GitHub App user access tokens](https://docs.github.com/en/enterprise-server@3.12/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [Fine-grained personal access tokens](https://docs.github.com/en/enterprise-server@3.12/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-fine-grained-personal-access-token)

The fine-grained token must have the following permission set:

- "Starring" user permissions (write) and "Metadata" repository permissions (read)

### [Parameters for "Star a repository for the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#star-a-repository-for-the-authenticated-user--parameters)

| Name, Type, Description                                              |
| -------------------------------------------------------------------- |
| accept string Setting to application/vnd.github+json is recommended. |

| Name, Type, Description                                                                                    |
| ---------------------------------------------------------------------------------------------------------- |
| owner string RequiredThe account owner of the repository. The name is not case sensitive.                  |
| repo string RequiredThe name of the repository without the .git extension. The name is not case sensitive. |

### [HTTP response status codes for "Star a repository for the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#star-a-repository-for-the-authenticated-user--status-codes)

| Status code | Description             |
| ----------- | ----------------------- |
| 204         | No Content              |
| 304         | Not modified            |
| 401         | Requires authentication |
| 403         | Forbidden               |
| 404         | Resource not found      |

### [Code samples for "Star a repository for the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#star-a-repository-for-the-authenticated-user--code-samples)

#### Request example

put/user/starred/{owner}/{repo}[cURL](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[JavaScript](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[GitHub CLI](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)Copy to clipboard curl request example`curl -L \
  -X PUT \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer <YOUR-TOKEN>" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  http(s)://HOSTNAME/api/v3/user/starred/OWNER/REPO`#### Response

`Status: 204`## [Unstar a repository for the authenticated user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#unstar-a-repository-for-the-authenticated-user)

Unstar a repository that the authenticated user has previously starred.

### [Fine-grained access tokens for "Unstar a repository for the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#unstar-a-repository-for-the-authenticated-user--fine-grained-access-tokens)

This endpoint works with the following fine-grained token types:

- [GitHub App user access tokens](https://docs.github.com/en/enterprise-server@3.12/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [Fine-grained personal access tokens](https://docs.github.com/en/enterprise-server@3.12/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-fine-grained-personal-access-token)

The fine-grained token must have the following permission set:

- "Starring" user permissions (write) and "Metadata" repository permissions (read)

### [Parameters for "Unstar a repository for the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#unstar-a-repository-for-the-authenticated-user--parameters)

| Name, Type, Description                                              |
| -------------------------------------------------------------------- |
| accept string Setting to application/vnd.github+json is recommended. |

| Name, Type, Description                                                                                    |
| ---------------------------------------------------------------------------------------------------------- |
| owner string RequiredThe account owner of the repository. The name is not case sensitive.                  |
| repo string RequiredThe name of the repository without the .git extension. The name is not case sensitive. |

### [HTTP response status codes for "Unstar a repository for the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#unstar-a-repository-for-the-authenticated-user--status-codes)

| Status code | Description             |
| ----------- | ----------------------- |
| 204         | No Content              |
| 304         | Not modified            |
| 401         | Requires authentication |
| 403         | Forbidden               |
| 404         | Resource not found      |

### [Code samples for "Unstar a repository for the authenticated user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#unstar-a-repository-for-the-authenticated-user--code-samples)

#### Request example

delete/user/starred/{owner}/{repo}[cURL](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[JavaScript](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[GitHub CLI](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)Copy to clipboard curl request example`curl -L \
  -X DELETE \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer <YOUR-TOKEN>" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  http(s)://HOSTNAME/api/v3/user/starred/OWNER/REPO`#### Response

`Status: 204`## [List repositories starred by a user](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-a-user)

Lists repositories a user has starred.

This endpoint supports the following custom media types. For more information, see "[Media types](https://docs.github.com/enterprise-server@3.12/rest/using-the-rest-api/getting-started-with-the-rest-api#media-types)."

- **`application/vnd.github.star+json`**: Includes a timestamp of when the star was created.

### [Fine-grained access tokens for "List repositories starred by a user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-a-user--fine-grained-access-tokens)

This endpoint works with the following fine-grained token types:

- [GitHub App user access tokens](https://docs.github.com/en/enterprise-server@3.12/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [GitHub App installation access tokens](https://docs.github.com/en/enterprise-server@3.12/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app)
- [Fine-grained personal access tokens](https://docs.github.com/en/enterprise-server@3.12/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-fine-grained-personal-access-token)

The fine-grained token must have the following permission set:

- "Starring" user permissions (read)

This endpoint can be used without authentication or the aforementioned permissions if only public resources are requested.

### [Parameters for "List repositories starred by a user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-a-user--parameters)

| Name, Type, Description                                              |
| -------------------------------------------------------------------- |
| accept string Setting to application/vnd.github+json is recommended. |

| Name, Type, Description                                         |
| --------------------------------------------------------------- |
| username string RequiredThe handle for the GitHub user account. |

| Name, Type, Description                                                                                                                                                                              |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| sort string The property to sort the results by. created means when the repository was starred. updated means when the repository was last pushed to.Default: createdCan be one of: created, updated |
| direction string The direction to sort the results by.Default: descCan be one of: asc, desc                                                                                                          |
| per_page integer The number of results per page (max 100). For more information, see "Using pagination in the REST API."Default: 30                                                                  |
| page integer The page number of the results to fetch. For more information, see "Using pagination in the REST API."Default: 1                                                                        |

### [HTTP response status codes for "List repositories starred by a user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-a-user--status-codes)

| Status code | Description |
| ----------- | ----------- |
| 200         | OK          |

### [Code samples for "List repositories starred by a user"](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#list-repositories-starred-by-a-user--code-samples)

#### Request example

get/users/{username}/starred[cURL](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[JavaScript](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[GitHub CLI](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)Copy to clipboard curl request example`curl -L \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer <YOUR-TOKEN>" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  http(s)://HOSTNAME/api/v3/users/USERNAME/starred`#### Default response

[Example response](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)[Response schema](https://docs.github.com/en/enterprise-server@3.12/rest/activity/#)`Status: 200``[
  {
    "id": 1296269,
    "node_id": "MDEwOlJlcG9zaXRvcnkxMjk2MjY5",
    "name": "Hello-World",
    "full_name": "octocat/Hello-World",
    "owner": {
      "login": "octocat",
      "id": 1,
      "node_id": "MDQ6VXNlcjE=",
      "avatar_url": "https://github.com/images/error/octocat_happy.gif",
      "gravatar_id": "",
      "url": "https://HOSTNAME/users/octocat",
      "html_url": "https://github.com/octocat",
      "followers_url": "https://HOSTNAME/users/octocat/followers",
      "following_url": "https://HOSTNAME/users/octocat/following{/other_user}",
      "gists_url": "https://HOSTNAME/users/octocat/gists{/gist_id}",
      "starred_url": "https://HOSTNAME/users/octocat/starred{/owner}{/repo}",
      "subscriptions_url": "https://HOSTNAME/users/octocat/subscriptions",
      "organizations_url": "https://HOSTNAME/users/octocat/orgs",
      "repos_url": "https://HOSTNAME/users/octocat/repos",
      "events_url": "https://HOSTNAME/users/octocat/events{/privacy}",
      "received_events_url": "https://HOSTNAME/users/octocat/received_events",
      "type": "User",
      "site_admin": false
    },
    "private": false,
    "html_url": "https://github.com/octocat/Hello-World",
    "description": "This your first repo!",
    "fork": false,
    "url": "https://HOSTNAME/repos/octocat/Hello-World",
    "archive_url": "https://HOSTNAME/repos/octocat/Hello-World/{archive_format}{/ref}",
    "assignees_url": "https://HOSTNAME/repos/octocat/Hello-World/assignees{/user}",
    "blobs_url": "https://HOSTNAME/repos/octocat/Hello-World/git/blobs{/sha}",
    "branches_url": "https://HOSTNAME/repos/octocat/Hello-World/branches{/branch}",
    "collaborators_url": "https://HOSTNAME/repos/octocat/Hello-World/collaborators{/collaborator}",
    "comments_url": "https://HOSTNAME/repos/octocat/Hello-World/comments{/number}",
    "commits_url": "https://HOSTNAME/repos/octocat/Hello-World/commits{/sha}",
    "compare_url": "https://HOSTNAME/repos/octocat/Hello-World/compare/{base}...{head}",
    "contents_url": "https://HOSTNAME/repos/octocat/Hello-World/contents/{+path}",
    "contributors_url": "https://HOSTNAME/repos/octocat/Hello-World/contributors",
    "deployments_url": "https://HOSTNAME/repos/octocat/Hello-World/deployments",
    "downloads_url": "https://HOSTNAME/repos/octocat/Hello-World/downloads",
    "events_url": "https://HOSTNAME/repos/octocat/Hello-World/events",
    "forks_url": "https://HOSTNAME/repos/octocat/Hello-World/forks",
    "git_commits_url": "https://HOSTNAME/repos/octocat/Hello-World/git/commits{/sha}",
    "git_refs_url": "https://HOSTNAME/repos/octocat/Hello-World/git/refs{/sha}",
    "git_tags_url": "https://HOSTNAME/repos/octocat/Hello-World/git/tags{/sha}",
    "git_url": "git:github.com/octocat/Hello-World.git",
    "issue_comment_url": "https://HOSTNAME/repos/octocat/Hello-World/issues/comments{/number}",
    "issue_events_url": "https://HOSTNAME/repos/octocat/Hello-World/issues/events{/number}",
    "issues_url": "https://HOSTNAME/repos/octocat/Hello-World/issues{/number}",
    "keys_url": "https://HOSTNAME/repos/octocat/Hello-World/keys{/key_id}",
    "labels_url": "https://HOSTNAME/repos/octocat/Hello-World/labels{/name}",
    "languages_url": "https://HOSTNAME/repos/octocat/Hello-World/languages",
    "merges_url": "https://HOSTNAME/repos/octocat/Hello-World/merges",
    "milestones_url": "https://HOSTNAME/repos/octocat/Hello-World/milestones{/number}",
    "notifications_url": "https://HOSTNAME/repos/octocat/Hello-World/notifications{?since,all,participating}",
    "pulls_url": "https://HOSTNAME/repos/octocat/Hello-World/pulls{/number}",
    "releases_url": "https://HOSTNAME/repos/octocat/Hello-World/releases{/id}",
    "ssh_url": "git@github.com:octocat/Hello-World.git",
    "stargazers_url": "https://HOSTNAME/repos/octocat/Hello-World/stargazers",
    "statuses_url": "https://HOSTNAME/repos/octocat/Hello-World/statuses/{sha}",
    "subscribers_url": "https://HOSTNAME/repos/octocat/Hello-World/subscribers",
    "subscription_url": "https://HOSTNAME/repos/octocat/Hello-World/subscription",
    "tags_url": "https://HOSTNAME/repos/octocat/Hello-World/tags",
    "teams_url": "https://HOSTNAME/repos/octocat/Hello-World/teams",
    "trees_url": "https://HOSTNAME/repos/octocat/Hello-World/git/trees{/sha}",
    "clone_url": "https://github.com/octocat/Hello-World.git",
    "mirror_url": "git:git.example.com/octocat/Hello-World",
    "hooks_url": "https://HOSTNAME/repos/octocat/Hello-World/hooks",
    "svn_url": "https://svn.github.com/octocat/Hello-World",
    "homepage": "https://github.com",
    "language": null,
    "forks_count": 9,
    "stargazers_count": 80,
    "watchers_count": 80,
    "size": 108,
    "default_branch": "master",
    "open_issues_count": 0,
    "is_template": true,
    "topics": [
      "octocat",
      "atom",
      "electron",
      "api"
    ],
    "has_issues": true,
    "has_projects": true,
    "has_wiki": true,
    "has_pages": false,
    "has_downloads": true,
    "archived": false,
    "disabled": false,
    "visibility": "public",
    "pushed_at": "2011-01-26T19:06:43Z",
    "created_at": "2011-01-26T19:01:12Z",
    "updated_at": "2011-01-26T19:14:43Z",
    "permissions": {
      "admin": false,
      "push": false,
      "pull": true
    },
    "allow_rebase_merge": true,
    "template_repository": null,
    "temp_clone_token": "ABTLWHOULUVAXGTRYU7OC2876QJ2O",
    "allow_squash_merge": true,
    "allow_auto_merge": false,
    "delete_branch_on_merge": true,
    "allow_merge_commit": true,
    "subscribers_count": 42,
    "network_count": 0,
    "license": {
      "key": "mit",
      "name": "MIT License",
      "url": "https://HOSTNAME/licenses/mit",
      "spdx_id": "MIT",
      "node_id": "MDc6TGljZW5zZW1pdA==",
      "html_url": "https://github.com/licenses/mit"
    },
    "forks": 1,
    "open_issues": 1,
    "watchers": 1
  }
]`## Help and support

### Still need help?

[Ask the GitHub community](https://github.com/orgs/community/discussions)[Contact support](https://support.github.com)## Legal

- © 2025 GitHub, Inc.
- [Terms](https://docs.github.com/en/site-policy/github-terms/github-terms-of-service)
- [Privacy](https://docs.github.com/en/site-policy/privacy-policies/github-privacy-statement)
- [Status](https://www.githubstatus.com/)
- [Pricing](https://github.com/pricing)
- [Expert services](https://services.github.com)
- [Blog](https://github.blog)

## Media links

- <https://github.github.com/docs-ghes-3.12/assets/cb-345/images/site/favicon.png>
- <https://github.github.com/docs-ghes-3.12/assets/cb-345/images/social-cards/default.png>
- <https://docs.github.com/assets/cb-345/images/social-cards/default.png>
- <https://github.com/images/error/octocat_happy.gif>
