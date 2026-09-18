export interface ChatEntry {
  role: "user" | "assistant";
  text: string;
}

export default function ChatLog({ entries }: { entries: ChatEntry[] }) {
  return (
    <div className="chat-log">
      {entries.length === 0 && (
        <p className="empty-hint">Hold the button and talk to Hanna.</p>
      )}
      {entries.map((entry, i) => (
        <div key={i} className={`bubble bubble-${entry.role}`}>
          {entry.text}
        </div>
      ))}
    </div>
  );
}
