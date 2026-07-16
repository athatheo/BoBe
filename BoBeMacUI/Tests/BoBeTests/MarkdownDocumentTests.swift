@testable import BoBe
import Testing

@Suite("Guided Markdown editor")
struct MarkdownDocumentTests {
    @Test
    func parsesAndRendersStructuredDocuments() {
        let source = """
        # About me

        A short introduction.

        ## Preferences

        Concise answers.

        ## Work

        Building BoBe.
        """

        let draft = MarkdownDocumentDraft.parse(source, fallbackTitle: "Profile")

        #expect(draft.title == "About me")
        #expect(draft.introduction == "A short introduction.")
        #expect(draft.sections.map(\.title) == ["Preferences", "Work"])
        #expect(draft.render().contains("## Work\n\nBuilding BoBe."))
    }

    @Test
    func fallsBackForPlainProse() {
        let draft = MarkdownDocumentDraft.parse("Helpful and concise.", fallbackTitle: "Soul")

        #expect(draft.title == "Soul")
        #expect(draft.introduction == "Helpful and concise.")
        #expect(draft.sections.isEmpty)
    }

    @Test
    func promotesFriendlyColonSectionsAndPreservesInlineMarkdown() {
        let source = """
        # Character

        **Warm** and concise.

        When helping:
        - Be direct
        - Admit uncertainty
        """

        let draft = MarkdownDocumentDraft.parse(source, fallbackTitle: "Soul")

        #expect(draft.introduction == "**Warm** and concise.")
        #expect(draft.sections.map(\.title) == ["When helping"])
        #expect(draft.sections.first?.body == "- Be direct\n- Admit uncertainty")
        #expect(draft.render().contains("**Warm** and concise."))
    }

    @Test
    func preservesAllInlineMarkdownDuringGuidedRoundTrip() {
        let source = """
        # Profile

        I use *emphasis*, **strong text**, `code`, and [links](https://example.com).

        ## Notes

        ~~Keep syntax~~ and ![images](image.png).
        """
        let draft = MarkdownDocumentDraft.parse(source, fallbackTitle: "Profile")
        let rendered = draft.render()
        #expect(rendered.contains("*emphasis*"))
        #expect(rendered.contains("**strong text**"))
        #expect(rendered.contains("`code`"))
        #expect(rendered.contains("[links](https://example.com)"))
        #expect(rendered.contains("![images](image.png)"))
    }

    @Test
    func preservesHeadingLikeLinesInsideFencedCode() {
        let source = """
        # Profile

        ## Notes

        ```md
        ## Example heading
        ```
        """
        let rendered = MarkdownDocumentDraft.parse(source, fallbackTitle: "Profile").render()
        #expect(rendered.contains("```md\n## Example heading\n```"))
    }
}
