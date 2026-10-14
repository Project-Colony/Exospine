# Fonts

The three font families every Colony program uses, and only those three
(Project-Colony-Resources, `design/typography.md`), loaded by
`styles/fonts.css`.

| Directory | Family | Files | Version | Licence |
|---|---|---|---|---|
| `JetBrainsMonoNerdFont/` | `JetBrainsMono Nerd Font` | Regular, Medium, Bold | JetBrains Mono 2.304, Nerd Fonts 3.4.0 | SIL OFL 1.1, `JetBrainsMonoNerdFont/OFL.txt` |
| `OpenDyslexic/` | `OpenDyslexic` | Regular | 0.990 | SIL OFL 1.1, `OpenDyslexic/OFL.txt` |
| `FontAwesome/` | `Font Awesome 6 Free` | Solid | 6.7.2 | SIL OFL 1.1, `FontAwesome/OFL.txt` |

They are the files `colony-ui` ships (`crates/colony-ui/assets/fonts/` in
Project-Colony-Resources), converted to WOFF2 with `woff2_compress`, which
changes the container and not the font data. Sources:

- JetBrains Mono: <https://github.com/JetBrains/JetBrainsMono>, patched with the
  Nerd Fonts glyphs: <https://github.com/ryanoasis/nerd-fonts>
- OpenDyslexic: <https://github.com/antijingoist/opendyslexic>
- Font Awesome Free: <https://github.com/FortAwesome/Font-Awesome>

The SIL Open Font License lets these files be bundled and redistributed with
software, including GPL software like Exospine, as long as each font keeps its
licence text and is not sold on its own.
