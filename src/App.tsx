import { useCallback, useEffect, useRef, useState } from "react";
import {
  startRecording,
  stopRecording,
  transcribe,
  askLlm,
  speak,
  initMemory,
  LlamaMessage,
} from "./lib/api";
import { routeUtterance } from "./lib/router";
import { answerFromNotes } from "./lib/rag";
import { buildMemoryContext, maybeDistill, recordTurn } from "./lib/personality";
import ChatLog, { ChatEntry } from "./components/ChatLog";
import MicButton from "./components/MicButton";
import TextInput from "./components/TextInput";

// Adjust to wherever your personal notes/docs live.
const NOTES_DIR = "/home/carlo/Notes";

const SYSTEM_PROMPT: LlamaMessage = {
  role: "system",
  content:
    "You are Hanna, Carlo's personal offline voice assistant. Keep answers short and conversational — this will be read aloud by text-to-speech, so avoid lists, markdown, and long tangents.",
};

export default function App() {
  const [log, setLog] = useState<ChatEntry[]>([]);
  const [status, setStatus] = useState<"idle" | "recording" | "thinking" | "speaking">("idle");
  const historyRef = useRef<LlamaMessage[]>([SYSTEM_PROMPT]);
  const recordingPathRef = useRef<string>("");

  useEffect(() => {
    initMemory().catch((err) => console.error("memory init failed:", err));
  }, []);

  const appendEntry = (entry: ChatEntry) =>
    setLog((prev) => [...prev, entry]);

  const processUtterance = useCallback(async (text: string) => {
    appendEntry({ role: "user", text });
    await recordTurn("user", text);

    const decision = await routeUtterance(text);

    let replyText: string;
    if (decision.kind === "handled") {
      replyText = decision.reply;
    } else if (decision.kind === "rag") {
      replyText = await answerFromNotes(decision.prompt, NOTES_DIR);
    } else {
      const memoryContext = await buildMemoryContext(decision.prompt);
      const turnHistory: LlamaMessage[] = memoryContext
        ? [
            ...historyRef.current,
            { role: "system", content: memoryContext },
            { role: "user", content: decision.prompt },
          ]
        : [...historyRef.current, { role: "user", content: decision.prompt }];

      replyText = await askLlm(turnHistory);

      historyRef.current.push({ role: "user", content: decision.prompt });
      historyRef.current.push({ role: "assistant", content: replyText });

      void maybeDistill(historyRef.current);
    }

    appendEntry({ role: "assistant", text: replyText });
    await recordTurn("assistant", replyText);

    setStatus("speaking");
    try {
      await speak(replyText);
    } catch (err) {
      console.error("TTS failed (reply text still shown):", err);
    }
    setStatus("idle");
  }, []);

  const handlePressStart = useCallback(async () => {
    setStatus("recording");
    try {
      recordingPathRef.current = await startRecording();
    } catch (err) {
      recordingPathRef.current = "";
      appendEntry({ role: "assistant", text: `Couldn't start recording: ${err}` });
      setStatus("idle");
    }
  }, []);

  const handlePressEnd = useCallback(async () => {
    if (!recordingPathRef.current) {
      setStatus("idle");
      return;
    }
    try {
      await stopRecording();
      setStatus("thinking");

      const text = await transcribe(recordingPathRef.current);
      if (!text.trim()) {
        setStatus("idle");
        return;
      }

      await processUtterance(text);
    } catch (err) {
      appendEntry({ role: "assistant", text: `Something went wrong: ${err}` });
      setStatus("idle");
    }
  }, [processUtterance]);

  const handleTextSubmit = useCallback(
    async (text: string) => {
      setStatus("thinking");
      try {
        await processUtterance(text);
      } catch (err) {
        appendEntry({ role: "assistant", text: `Something went wrong: ${err}` });
        setStatus("idle");
      }
    },
    [processUtterance]
  );

  const busy = status === "thinking" || status === "speaking";

  return (
    <div className="app">
      <header>
        <h1>Hanna</h1>
        <span className={`status status-${status}`}>{status}</span>
      </header>
      <ChatLog entries={log} />
      <TextInput disabled={busy} onSubmit={handleTextSubmit} />
      <MicButton
        status={status}
        onPressStart={handlePressStart}
        onPressEnd={handlePressEnd}
      />
    </div>
  );
}