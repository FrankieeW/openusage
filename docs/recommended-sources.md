# Recommended Sources

Plugin sources you can add in the Hub (**Add Source**). Add one by pasting its
URL into **Add Source**.

Last checked: 2026-06-17.

For install checks and source trust labels, see [Hub Safety Model](hub-safety-model.md).

## Curated Sources

| Source | URL | Notes |
| --- | --- | --- |
| Frankie's Collection | https://github.com/FrankieeW/openusage-collection | Default community collection. Includes upstream-style plugins plus extra providers such as DeepSeek and New API. |
| Upstream | https://github.com/robinebers/openusage | Original OpenUsage project. Useful as the upstream reference source. |

## Community Sources

These sources are public community work. Hub labels user-added GitHub sources as
**Community**. Add them only if you want the listed providers and are
comfortable trusting that source.

| Provider | Source | Notes |
| --- | --- | --- |
| Multiple | https://github.com/barramee27/crossusage | Large community source with extra providers including Ollama, Fireworks AI, Command Code, CrofAI, Neuralwatt, and Cursor Nightly. |
| OpenRouter | https://github.com/ExTV/openusage/tree/feat/openrouter-provider | Branch source for OpenRouter. Recently synced with upstream when checked. |
| Warp | https://github.com/JulianKniephoff/openusage/tree/feat/warp | Branch source for Warp. Also includes Gemini and Windsurf. |
| Pioneer | https://github.com/andrew54068/openusage/tree/feat/pioneer-plugin | Branch source for Pioneer. Also includes Gemini and Windsurf. |
| Zed AI | https://github.com/rohithgoud30/openusage/tree/add-zed-ai-support | Branch source for Zed AI. |

## Source Identity

Some sources point at a branch, such as:

```text
https://github.com/ExTV/openusage/tree/feat/openrouter-provider
```

Hub should keep the branch as part of the source. If the branch is dropped, Hub
may load the repository's default branch and miss the intended plugin.

Different sources may publish the same `pluginId` and `version`. Hub compares
the package hash of each `plugins/<id>/` directory to tell whether they are the
same package or different packages with the same name.

Non-GitHub git URLs are labeled **Unknown Git Source** and require a preview
confirmation before install.

## Find More Forks

Browse the public forks of these projects to discover new sources and add them
manually via **Add Source**.

- Forks of upstream: https://github.com/robinebers/openusage/forks
- Forks of Frankie's Collection: https://github.com/FrankieeW/openusage-collection/forks

## Contributing

Found a useful source? Open a PR adding a row to the table above. Keep the
notes honest. Mention the provider, source URL, and whether you have installed
and run the plugin locally.
