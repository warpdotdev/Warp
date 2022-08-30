<p align="center">
    <a href="https://app.warp.dev/get_warp">
    <img width="600" alt="horz - dark" src="https://user-images.githubusercontent.com/29553206/161685377-cb458631-eb2e-454f-aab7-3f5bfec745ee.png">
    </a>
</p>

<p align="center">
  <a href="https://warp.dev">Website</a>
  ·
  <a href="#installation">Installation</a>
  ·
  <a href="https://warp.dev/blog">Blog</a>
</p>

<a href="https://www.youtube.com/watch?v=T7R8lvvBgOI">
    <img width="1025" alt="Screen Shot 2022-04-05 at 01 59 53" src="https://user-images.githubusercontent.com/29553206/161688541-2889478f-d02e-497c-8340-41569e579a42.png">
</a>

<h1></h1>

## About

This is an issues-only repo for [Warp](https://www.warp.dev), a [blazingly-fast modern Rust based GPU-accelerated terminal](https://www.warp.dev/blog/how-warp-works) built to make [you and your team more productive.](https://www.warp.dev/blog/how-we-design-warp-our-product-philosophy)

## Supported Platforms

As of April 5th, 2022, Warp is available to all macOS users, without joining a waitlist.

We are calling this new phase of the product our “public beta” – it’s a “beta” because we know there are still some issues to smooth out, but we are confident that even today the experience is meaningfully better than in other terminals.

We have plans to support [Linux](https://github.com/warpdotdev/Warp/issues/120), [Windows,](https://github.com/warpdotdev/Warp/issues/204) and the Web (WASM)!

## Installation

You can [download Warp](https://app.warp.dev/get_warp) from our website (<https://warp.dev>) or via Homebrew:

```shell
brew install --cask warp
```

## Changelog and Releases

We try to release an update every Warp Wednesday. See our [changelog (release notes).](https://docs.warp.dev/help/changelog)

## Issues, Bugs, and Feature Requests

File issue requests [in this repo!](https://github.com/warpdotdev/warp/issues/new/choose)
We kindly ask that you please use our issue templates to make the issues easier to track for our team.

## Open Source & Contributing

We are planning to first open-source our Rust UI framework, and then parts and potentially all of our client codebase. The server portion of Warp will remain closed-source for now.

You can see how we’re thinking about open source here: [https://github.com/warpdotdev/Warp/discussions/400](https://github.com/warpdotdev/Warp/discussions/400)

As a side note, we are open sourcing our extension points as we go. The community has already been [contributing new themes](https://github.com/warpdotdev/themes). And we’ve just opened our [Workflows repository](https://github.com/warpdotdev/workflows) for the community to contribute common useful commands.

Interested in joining the team? See our [open roles](https://www.warp.dev/careers) and feel free to send us an email: hello at warpdotdev

## Support and Questions

1. See our [docs](https://docs.warp.dev/) for a walkthrough of the features within our app.
2. Join our [Discord](https://discord.gg/warpdotdev) to chat with other users and get immediate help with members of the Warp team.

For anything else, please don't hesitate to reach out via email at hello at warpdotdev

## Community Guidelines

At a high level, we ask everyone be respectful and empathetic. We follow the [Github Community Guidelines](https://docs.github.com/en/github/site-policy/github-community-guidelines):

* Be welcoming and open-minded
* Respect each other
* Communicate with empathy
* Be clear and stay on topic

## Open Source Dependencies

We'd like to call out a few of the [open source dependencies](https://docs.warp.dev/help/licenses) that have helped Warp to get off the ground:

* [Tokio](https://github.com/tokio-rs/tokio)
* [NuShell](https://github.com/nushell/nushell)
* [Fig Completion Specs](https://github.com/withfig/autocomplete)
* [Warp Server Framework](https://github.com/seanmonstar/warp)
* [Alacritty](https://github.com/alacritty/alacritty)
* [Hyper HTTP library](https://github.com/hyperium/hyper)
* [FontKit](https://github.com/servo/font-kit)
* [Core-foundation](https://github.com/servo/core-foundation-rs)
* [Smol](https://github.com/smol-rs/smol)
