interface Props {
  status: "idle" | "recording" | "thinking" | "speaking";
  onPressStart: () => void;
  onPressEnd: () => void;
}

export default function MicButton({ status, onPressStart, onPressEnd }: Props) {
  const label =
    status === "recording"
      ? "Listening… release to send"
      : status === "thinking"
      ? "Thinking…"
      : status === "speaking"
      ? "Speaking…"
      : "Hold to talk";

  return (
    <button
      className={`mic-button mic-${status}`}
      disabled={status === "thinking" || status === "speaking"}
      onMouseDown={onPressStart}
      onMouseUp={onPressEnd}
      onTouchStart={onPressStart}
      onTouchEnd={onPressEnd}
    >
      {label}
    </button>
  );
}
