@echo off
REM Starts everything Hanna needs, in the right order, every time.
REM Run this instead of manually starting llama-server + npm run tauri dev
REM separately — it's easy to forget --embeddings when doing it by hand,
REM and forgetting it silently breaks memory (no error until you ask
REM Hanna something and notice she doesn't remember).

setlocal

set LLAMA_SERVER=src-tauri\bin\llama-server.exe
set MODEL_PATH=src-tauri\models\hanna-llm.gguf

if not exist "%LLAMA_SERVER%" (
    echo ERROR: %LLAMA_SERVER% not found.
    echo Make sure llama-server.exe is in src-tauri\bin\.
    pause
    exit /b 1
)

if not exist "%MODEL_PATH%" (
    echo ERROR: %MODEL_PATH% not found.
    echo Make sure hanna-llm.gguf is in src-tauri\models\.
    pause
    exit /b 1
)

echo Starting llama-server (with --embeddings) in its own window...
start "Hanna LLM Server - do not close while using Hanna" cmd /k "%LLAMA_SERVER% -m %MODEL_PATH% --port 8080 --embeddings --pooling mean"
echo Waiting a few seconds for the server to come up...
timeout /t 5 /nobreak >nul

echo Starting Hanna...
call npm run tauri dev

REM When you close the Hanna app window, this script ends too, but the
REM llama-server window stays open on purpose — close it manually when
REM you're done, or leave it running for next time.
endlocal