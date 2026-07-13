# GitHub Search API: Get the number of stars for a repository · GitHub

Source: https://gist.github.com/jasonrudolph/6057563

[Skip to content](https://gist.github.com/jasonrudolph/#start-of-content) [

](https://gist.github.com/)
        Search Gists
Search Gists [All gists](https://gist.github.com/discover) [Back to GitHub](https://github.com) [
      Sign in
](https://gist.github.com/auth/github?return_to=https%3A%2F%2Fgist.github.com%2Fjasonrudolph%2F6057563) [
        Sign up
](https://gist.github.com/join?return_to=https%3A%2F%2Fgist.github.com%2Fjasonrudolph%2F6057563&source=header-gist) [

](https://gist.github.com/) [
        Sign in
](https://gist.github.com/auth/github?return_to=https%3A%2F%2Fgist.github.com%2Fjasonrudolph%2F6057563) [
          Sign up
](https://gist.github.com/join?return_to=https%3A%2F%2Fgist.github.com%2Fjasonrudolph%2F6057563&source=header-gist) You signed in with another tab or window. Reload to refresh your session. You signed out in another tab or window. Reload to refresh your session. You switched accounts on another tab or window. Reload to refresh your session. Dismiss alert
        Instantly share code, notes, and snippets.

[![@jasonrudolph](https://avatars.githubusercontent.com/u/2988?s=64&v=4)](https://gist.github.com/jasonrudolph) #
            [jasonrudolph](https://gist.github.com/jasonrudolph)/**[gist:6057563](https://gist.github.com/jasonrudolph/6057563)**

          Last active
          March 2, 2026 04:04
Show Gist options -

    [

          Download ZIP

](https://gist.github.com/jasonrudolph/6057563/archive/f48ebd9b02fb16ac9c0a27a883a146c45bea0c90.zip)

-
          [  Star 67 (67)
](https://gist.github.com/login?return_to=https%3A%2F%2Fgist.github.com%2Fjasonrudolph%2F6057563)You must be signed in to star a gist

-
            [  Fork 9 (9)
](https://gist.github.com/login?return_to=https%3A%2F%2Fgist.github.com%2Fjasonrudolph%2F6057563)You must be signed in to fork a gist

-

Embed #
        Select an option

-

           Embed
Embed this gist in your website.

-

           Share
Copy sharable link for this gist.

-

          Clone via HTTPS
Clone using the web URL.

## No results found

[Learn more about clone URLs](https://docs.github.com/articles/which-remote-url-should-i-use)
        Clone this repository at <script src="https://gist.github.com/jasonrudolph/6057563.js"></script>

-
        Save jasonrudolph/6057563 to your computer and use it in GitHub Desktop.

[

      Code
](https://gist.github.com/jasonrudolph/6057563) [

        Revisions
        3
](https://gist.github.com/jasonrudolph/6057563/revisions) [

        Stars
        67
](https://gist.github.com/jasonrudolph/6057563/stargazers) [

        Forks
        9
](https://gist.github.com/jasonrudolph/6057563/forks) Embed #
        Select an option

-

           Embed
Embed this gist in your website.

-

           Share
Copy sharable link for this gist.

-

          Clone via HTTPS
Clone using the web URL.

## No results found

[Learn more about clone URLs](https://docs.github.com/articles/which-remote-url-should-i-use)
        Clone this repository at <script src="https://gist.github.com/jasonrudolph/6057563.js"></script>
Save jasonrudolph/6057563 to your computer and use it in GitHub Desktop. [Download ZIP](https://gist.github.com/jasonrudolph/6057563/archive/f48ebd9b02fb16ac9c0a27a883a146c45bea0c90.zip)
    GitHub Search API: Get the number of stars for a repository
  [  Raw
](https://gist.github.com/jasonrudolph/6057563/raw/f48ebd9b02fb16ac9c0a27a883a146c45bea0c90/gistfile1.md) [
            **
              gistfile1.md
            **
          ](https://gist.github.com/jasonrudolph/#file-gistfile1-md) James Sugrue [asked](https://twitter.com/sugrue/status/359402875761340417), "@GitHubAPI is there a way to find the number of stars for a given repository?"

## Example

[](https://gist.github.com/jasonrudolph/#example) ```
$ curl -ni "https://api.github.com/search/repositories?q=more+useful+keyboard" -H 'Accept: application/vnd.github.preview'
```

```
{
  "total_count": 1,
  "items": [
    {
      "id": 9118195,
      "name": "keyboard",
      "full_name": "jasonrudolph/keyboard",
      "owner": {
        "login": "jasonrudolph",
        "id": 2988,
        "avatar_url": "https://secure.gravatar.com/avatar/592e1e6f041f9a4ec51846fd82013aea?d=https://a248.e.akamai.net/assets.github.com%2Fimages%2Fgravatars%2Fgravatar-user-420.png",
        "gravatar_id": "592e1e6f041f9a4ec51846fd82013aea",
        "url": "https://api.github.com/users/jasonrudolph",
        "html_url": "https://github.com/jasonrudolph",
        "followers_url": "https://api.github.com/users/jasonrudolph/followers",
        "following_url": "https://api.github.com/users/jasonrudolph/following{/other_user}",
        "gists_url": "https://api.github.com/users/jasonrudolph/gists{/gist_id}",
        "starred_url": "https://api.github.com/users/jasonrudolph/starred{/owner}{/repo}",
        "subscriptions_url": "https://api.github.com/users/jasonrudolph/subscriptions",
        "organizations_url": "https://api.github.com/users/jasonrudolph/orgs",
        "repos_url": "https://api.github.com/users/jasonrudolph/repos",
        "events_url": "https://api.github.com/users/jasonrudolph/events{/privacy}",
        "received_events_url": "https://api.github.com/users/jasonrudolph/received_events",
        "type": "User"
      },
      "private": false,
      "html_url": "https://github.com/jasonrudolph/keyboard",
      "description": "Toward a more useful keyboard",
      "fork": false,
      "url": "https://api.github.com/repos/jasonrudolph/keyboard",
      "forks_url": "https://api.github.com/repos/jasonrudolph/keyboard/forks",
      "keys_url": "https://api.github.com/repos/jasonrudolph/keyboard/keys{/key_id}",
      "collaborators_url": "https://api.github.com/repos/jasonrudolph/keyboard/collaborators{/collaborator}",
      "teams_url": "https://api.github.com/repos/jasonrudolph/keyboard/teams",
      "hooks_url": "https://api.github.com/repos/jasonrudolph/keyboard/hooks",
      "issue_events_url": "https://api.github.com/repos/jasonrudolph/keyboard/issues/events{/number}",
      "events_url": "https://api.github.com/repos/jasonrudolph/keyboard/events",
      "assignees_url": "https://api.github.com/repos/jasonrudolph/keyboard/assignees{/user}",
      "branches_url": "https://api.github.com/repos/jasonrudolph/keyboard/branches{/branch}",
      "tags_url": "https://api.github.com/repos/jasonrudolph/keyboard/tags",
      "blobs_url": "https://api.github.com/repos/jasonrudolph/keyboard/git/blobs{/sha}",
      "git_tags_url": "https://api.github.com/repos/jasonrudolph/keyboard/git/tags{/sha}",
      "git_refs_url": "https://api.github.com/repos/jasonrudolph/keyboard/git/refs{/sha}",
      "trees_url": "https://api.github.com/repos/jasonrudolph/keyboard/git/trees{/sha}",
      "statuses_url": "https://api.github.com/repos/jasonrudolph/keyboard/statuses/{sha}",
      "languages_url": "https://api.github.com/repos/jasonrudolph/keyboard/languages",
      "stargazers_url": "https://api.github.com/repos/jasonrudolph/keyboard/stargazers",
      "contributors_url": "https://api.github.com/repos/jasonrudolph/keyboard/contributors",
      "subscribers_url": "https://api.github.com/repos/jasonrudolph/keyboard/subscribers",
      "subscription_url": "https://api.github.com/repos/jasonrudolph/keyboard/subscription",
      "commits_url": "https://api.github.com/repos/jasonrudolph/keyboard/commits{/sha}",
      "git_commits_url": "https://api.github.com/repos/jasonrudolph/keyboard/git/commits{/sha}",
      "comments_url": "https://api.github.com/repos/jasonrudolph/keyboard/comments{/number}",
      "issue_comment_url": "https://api.github.com/repos/jasonrudolph/keyboard/issues/comments/{number}",
      "contents_url": "https://api.github.com/repos/jasonrudolph/keyboard/contents/{+path}",
      "compare_url": "https://api.github.com/repos/jasonrudolph/keyboard/compare/{base}...{head}",
      "merges_url": "https://api.github.com/repos/jasonrudolph/keyboard/merges",
      "archive_url": "https://api.github.com/repos/jasonrudolph/keyboard/{archive_format}{/ref}",
      "downloads_url": "https://api.github.com/repos/jasonrudolph/keyboard/downloads",
      "issues_url": "https://api.github.com/repos/jasonrudolph/keyboard/issues{/number}",
      "pulls_url": "https://api.github.com/repos/jasonrudolph/keyboard/pulls{/number}",
      "milestones_url": "https://api.github.com/repos/jasonrudolph/keyboard/milestones{/number}",
      "notifications_url": "https://api.github.com/repos/jasonrudolph/keyboard/notifications{?since,all,participating}",
      "labels_url": "https://api.github.com/repos/jasonrudolph/keyboard/labels{/name}",
      "created_at": "2013-03-30T16:01:43Z",
      "updated_at": "2013-07-22T02:01:08Z",
      "pushed_at": "2013-07-14T00:26:07Z",
      "git_url": "git://github.com/jasonrudolph/keyboard.git",
      "ssh_url": "git@github.com:jasonrudolph/keyboard.git",
      "clone_url": "https://github.com/jasonrudolph/keyboard.git",
      "svn_url": "https://github.com/jasonrudolph/keyboard",
      "homepage": "",
      "size": 228,
      "watchers_count": 235,
      "language": null,
      "has_issues": true,
      "has_downloads": true,
      "has_wiki": false,
      "forks_count": 7,
      "mirror_url": null,
      "open_issues_count": 1,
      "forks": 7,
      "open_issues": 1,
      "watchers": 235,
      "master_branch": "master",
      "default_branch": "master",
      "score": 38.069878
    }
  ]
}
```

Stars and watchers are in a [transition period](http://developer.github.com/changes/2012-9-5-watcher-api/).
Until that transition is complete, you get the number of stars using the old terminology (i.e., "watchers_count").

[![@rr-paras-patel](https://avatars.githubusercontent.com/u/6395501?s=80&v=4)](https://gist.github.com/rr-paras-patel) ###
    **
            [rr-paras-patel](https://gist.github.com/rr-paras-patel)

      **

      commented

        [Jun 9, 2014](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=1242557#gistcomment-1242557)

              Copy link

                Copy Markdown

simple : [http://api.github.com/repos/[username]/[reponame]](http://api.github.com/repos/%5Busername%5D/%5Breponame%5D)

e.g. [https://api.github.com/repos/jasonrudolph/keyboard](https://api.github.com/repos/jasonrudolph/keyboard)

Consider "stargazers_count" field.

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@DONGChuan](https://avatars.githubusercontent.com/u/2586841?s=80&v=4)](https://gist.github.com/DONGChuan) ###
    **
            [DONGChuan](https://gist.github.com/DONGChuan)

      **

      commented

        [Nov 10, 2015](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=1617451#gistcomment-1617451)

              Copy link

                Copy Markdown

[@patelparas](https://github.com/patelparas) But how could we get directly the number with github api? It really costs sometimes to parse json ...

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@eliotsykes](https://avatars.githubusercontent.com/u/31698?s=80&v=4)](https://gist.github.com/eliotsykes) ###
    **
            [eliotsykes](https://gist.github.com/eliotsykes)

      **

      commented

        [Mar 26, 2016](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=1734545#gistcomment-1734545)

              Copy link

                Copy Markdown

[@DONGChuan](https://github.com/DONGChuan) - Using [jq](https://stedolan.github.io/jq/) to extract the `watcher_count` value for an API request for a single repo:

```
curl --silent 'https://api.github.com/repos/jasonrudolph/keyboard' -H 'Accept: application/vnd.github.preview' | jq '.watchers_count'
```

outputs the number of stars:

```
429
```

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@metrue](https://avatars.githubusercontent.com/u/1001246?s=80&v=4)](https://gist.github.com/metrue) ###
    **
            [metrue](https://gist.github.com/metrue)

      **

      commented

        [Aug 10, 2016](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=1846005#gistcomment-1846005)

              Copy link

                Copy Markdown

[@eliotsykes](https://github.com/eliotsykes)  this solutions is great, but GitHub api call rate limit is 6 per hour per IP if you are not authorized. [https://developer.github.com/v3/#rate-limiting](https://developer.github.com/v3/#rate-limiting)

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@BobRay](https://avatars.githubusercontent.com/u/335010?s=80&v=4)](https://gist.github.com/BobRay) ###
    **
            [BobRay](https://gist.github.com/BobRay)

      **

      commented

        [Jun 23, 2017](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=2131412#gistcomment-2131412) •
          edited

        Loading ###         Uh oh!

There was an error while loading. Please reload this page.

              Copy link

                Copy Markdown

I see there is now a stargazers_count which seems to always equal the watchers_count, but may diverge from it in the future.

BTW, getting an API key to increase the rate limit is free.

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@gabrielizalo](https://avatars.githubusercontent.com/u/656353?s=80&v=4)](https://gist.github.com/gabrielizalo) ###
    **
            [gabrielizalo](https://gist.github.com/gabrielizalo)

      **

      commented

        [Aug 1, 2018](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=2665959#gistcomment-2665959)

              Copy link

                Copy Markdown

Is there an example for GitHub API 4?

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@philwareham](https://avatars.githubusercontent.com/u/413665?s=80&v=4)](https://gist.github.com/philwareham) ###
    **
            [philwareham](https://gist.github.com/philwareham)

      **

      commented

        [Apr 24, 2019](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=2896930#gistcomment-2896930) •
          edited

        Loading ###         Uh oh!

There was an error while loading. Please reload this page.

              Copy link

                Copy Markdown

Just leaving this example here in case people want to find GitHub star count using GitHub API v4 (GraphQL), as the answer is sometimes not that easy to find...

```
query {
  repository(owner: your_username, name: your_repo_name) {
    stargazers {
      totalCount
    }
  }
}
```

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@victorbaranov](https://avatars.githubusercontent.com/u/16096224?s=80&v=4)](https://gist.github.com/victorbaranov) ###
    **
            [victorbaranov](https://gist.github.com/victorbaranov)

      **

      commented

        [May 30, 2019](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=2930728#gistcomment-2930728)

              Copy link

                Copy Markdown

> Just leaving this example here in case people want to find GitHub star count using GitHub API v4 (GraphQL), as the answer is sometimes not that easy to find...
>
> ```
> query {
>   repository(owner: your_username, name: your_repo_name) {
>     stargazers {
>       totalCount
>     }
>   }
> }
> ```

is it possible without GraphQL? (API only )

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@harshitsinghai77](https://avatars.githubusercontent.com/u/30886142?s=80&v=4)](https://gist.github.com/harshitsinghai77) ###
    **
            [harshitsinghai77](https://gist.github.com/harshitsinghai77)

      **

      commented

        [Dec 23, 2019](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=3119204#gistcomment-3119204)

              Copy link

                Copy Markdown

You can also check this [https://gist.github.com/harshitsinghai77/831aa8fa549d0ec5c89aad0065a186c8](https://gist.github.com/harshitsinghai77/831aa8fa549d0ec5c89aad0065a186c8) and read more about it here at [https://medium.com/@harshitsinghai77/demystifying-github-api-to-fetch-the-top-3-repositories-by-stars-using-node-js-aef8818551cb](https://medium.com/@harshitsinghai77/demystifying-github-api-to-fetch-the-top-3-repositories-by-stars-using-node-js-aef8818551cb)

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@VeraZab](https://avatars.githubusercontent.com/u/4257572?s=80&v=4)](https://gist.github.com/VeraZab) ###
    **
            [VeraZab](https://gist.github.com/VeraZab)

      **

      commented

        [Apr 22, 2020](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=3264806#gistcomment-3264806)

              Copy link

                Copy Markdown

this works well for me: curl --silent '[https://api.github.com/repos/plotly/dash](https://api.github.com/repos/plotly/dash)' | grep 'stargazers_count'

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@ikwyl6](https://avatars.githubusercontent.com/u/1643833?s=80&v=4)](https://gist.github.com/ikwyl6) ###
    **
            [ikwyl6](https://gist.github.com/ikwyl6)

      **

      commented

        [Feb 6, 2022](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=4054939#gistcomment-4054939)

              Copy link

                Copy Markdown

Why do you pass -H 'Accept: application/vnd.github.preview' ?

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@port19x](https://avatars.githubusercontent.com/u/82055622?s=80&v=4)](https://gist.github.com/port19x) ###
    **
            [port19x](https://gist.github.com/port19x)

      **

      commented

        [Apr 30, 2022](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=4150026#gistcomment-4150026)

              Copy link

                Copy Markdown

For just the number I came up with this

`curl -s "https://api.github.com/repos/pystardust/ani-cli" | grep stargazers_count | cut -d : -f 2 | tr -d " " | tr -d ","`

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@ishandutta2007](https://avatars.githubusercontent.com/u/2527354?s=80&v=4)](https://gist.github.com/ishandutta2007) ###
    **
            [ishandutta2007](https://gist.github.com/ishandutta2007)

      **

      commented

        [Oct 24, 2023](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=4736550#gistcomment-4736550) •
          edited

        Loading ###         Uh oh!

There was an error while loading. Please reload this page.

              Copy link

                Copy Markdown

I am trying to embed the same directly in markdown , so  I dont have the liberty to write custom js script.

For users I do this,

[ ![total stars](https://camo.githubusercontent.com/ec6437b8b1cfa1e7e20eab75a4f8ce9115af7e8d8cc1a5f94370b0d5bcd0ac67/68747470733a2f2f637573746f6d2d69636f6e2d6261646765732e6865726f6b756170702e636f6d2f62616467652f64796e616d69632f6a736f6e3f6c6f676f3d7374617226636f6c6f723d353539363063266c6162656c436f6c6f723d343838323037266c6162656c3d5374617273267374796c653d666f722d7468652d62616467652671756572793d2532342e73746172732675726c3d68747470733a2f2f6170692e6769746875622d737461722d636f756e7465722e776f726b6572732e6465762f757365722f696d617274696e657a)](https://github.com/imartinez?tab=repositories&sort=stargazers)

as the url is [https://api.github-star-counter.workers.dev/user/imartinez](https://api.github-star-counter.workers.dev/user/imartinez)

because this guy [idealcover](https://github.com/idealclover) has deployed a public cloudflare worker enpoint from this project [https://github.com/idealclover/GitHub-Star-Counter](https://github.com/idealclover/GitHub-Star-Counter)

For repos it doesn't work like this,

[

![total stars](https://camo.githubusercontent.com/dfcde0d036bde6748262a67a29a1456c031df5bb7dc7933361471b627f36ed90/68747470733a2f2f637573746f6d2d69636f6e2d6261646765732e6865726f6b756170702e636f6d2f62616467652f64796e616d69632f6a736f6e3f6c6f676f3d7374617226636f6c6f723d353539363063266c6162656c436f6c6f723d343838323037266c6162656c3d5374617273267374796c653d666f722d7468652d62616467652671756572793d2532342e73746172732675726c3d68747470733a2f2f6170692e6769746875622e636f6d2f7265706f732f696d617274696e657a2f70726976617465475054)

](https://github.com/imartinez/privateGPT)

with url as [https://api.github.com/repos/imartinez/privateGPT](https://api.github.com/repos/imartinez/privateGPT) because no one has any such worker which returns a json with star.

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[![@ishandutta2007](https://avatars.githubusercontent.com/u/2527354?s=80&v=4)](https://gist.github.com/ishandutta2007) ###
    **
            [ishandutta2007](https://gist.github.com/ishandutta2007)

      **

      commented

        [Oct 24, 2023](https://gist.github.com/jasonrudolph/6057563?permalink_comment_id=4736575#gistcomment-4736575)

              Copy link

                Copy Markdown

> For repos it doesn't work like this, [ ![total stars](https://camo.githubusercontent.com/d72807c79489cc802be425397dbfc6d6d291c529ca05df2240e8139b9655fcb5/68747470733a2f2f637573746f6d2d69636f6e2d6261646765732e6865726f6b756170702e636f6d2f62616467652f64796e616d69632f6a736f6e3f6c6f676f3d7374617226636f6c6f723d353539363063266c6162656c436f6c6f723d343838323037266c6162656c3d5374617273267374796c653d666f722d7468652d62616467652671756572793d2532342e73746172732675726c3d68747470733a2f2f6170692e6769746875622e636f6d2f7265706f732f696d617274696e657a2f70726976617465475054) ](https://github.com/imartinez/privateGPT)
>
> with url as [https://api.github.com/repos/imartinez/privateGPT](https://api.github.com/repos/imartinez/privateGPT) because no one has any such worker which returns a json with star.

Actually I had a gap in understanding, for repos it is easier.

[
![total stars](https://camo.githubusercontent.com/21f7fba869d5c1baf4ba6d5822f0ae028ebefd84213cb155e5495504bac59560/68747470733a2f2f637573746f6d2d69636f6e2d6261646765732e6865726f6b756170702e636f6d2f6769746875622f73746172732f696d617274696e657a2f707269766174654750543f6c6f676f3d7374617226636f6c6f723d353539363063266c6162656c436f6c6f723d343838323037266c6162656c3d5374617273267374796c653d666f722d7468652d62616467652671756572793d2532342e73746172732675726c3d68747470733a2f2f6170692e6769746875622e636f6d2f7265706f732f696d617274696e657a2f70726976617465475054)
](https://github.com/imartinez/privateGPT)

    Sorry, something went wrong.

###         Uh oh!

There was an error while loading. Please reload this page.

[Sign up for free](https://gist.github.com/join?source=comment-gist) **to join this conversation on GitHub**.
    Already have an account?
    [Sign in to comment](https://gist.github.com/login?return_to=https%3A%2F%2Fgist.github.com%2Fjasonrudolph%2F6057563) ## Footer

[

](https://github.com)
        © 2026 GitHub, Inc.
      ### Footer navigation

-
            [Terms](https://docs.github.com/site-policy/github-terms/github-terms-of-service)

-
            [Privacy](https://docs.github.com/site-policy/privacy-policies/github-privacy-statement)

-
            [Security](https://github.com/security)

-
            [Status](https://www.githubstatus.com/)

-
            [Community](https://github.community/)

-
            [Docs](https://docs.github.com/)

-
            [Contact](https://support.github.com?tags=dotcom-footer)

-

       Manage cookies

-

      Do not share my personal information

    You can’t perform that action at this time.

## Media links

- <https://gist.github.com/fluidicon.png>
- <https://github.githubassets.com/assets/gist-og-image-54fd7dc0713e.png>
- <https://github.githubassets.com/favicons/favicon.png>
