import { CodeBlock } from "./CodeBlock";

export function InlineMarkdown({ text }: { text: string }) {
  return text.split(/(`[^`]+`|\*\*[^*]+\*\*)/g).map((part, index) => {
    if (part.startsWith("`") && part.endsWith("`")) return <code key={index}>{part.slice(1, -1)}</code>;
    if (part.startsWith("**") && part.endsWith("**")) return <strong key={index}>{part.slice(2, -2)}</strong>;
    return part;
  });
}

export function MarkdownBody({ body }: { body: string }) {
  const blocks = body.split(/```([^\n`]*)\n?([\s\S]*?)```/g);

  return (
    <div className="markdown-body">
      {blocks.map((block, index) => {
        if (index % 3 === 1) return null;
        if (index % 3 === 2) {
          const language = blocks[index - 1].trim() || "text";
          return <CodeBlock key={index} code={block} language={language} />;
        }
        return block.split(/\n{2,}/).filter(Boolean).map((paragraph, paragraphIndex) => (
          <p key={`${index}-${paragraphIndex}`}><InlineMarkdown text={paragraph} /></p>
        ));
      })}
    </div>
  );
}
