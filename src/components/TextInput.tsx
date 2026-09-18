import { useState, FormEvent } from "react";

interface Props {
  disabled: boolean;
  onSubmit: (text: string) => void;
}

export default function TextInput({ disabled, onSubmit }: Props) {
  const [value, setValue] = useState("");

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    const trimmed = value.trim();
    if (!trimmed || disabled) return;
    onSubmit(trimmed);
    setValue("");
  };

  return (
    <form className="text-input-row" onSubmit={handleSubmit}>
      <input
        className="text-input"
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        placeholder="Or type to Hanna…"
        disabled={disabled}
      />
      <button className="text-send-button" type="submit" disabled={disabled || !value.trim()}>
        Send
      </button>
    </form>
  );
}