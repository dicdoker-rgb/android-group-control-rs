#!/usr/bin/env python3
import argparse
import os
import pathlib
import signal
import subprocess
import sys
import time


def write_pid(path: pathlib.Path, pid: int) -> None:
    path.write_text(str(pid))


def remove_if_exists(path: pathlib.Path) -> None:
    try:
        path.unlink()
    except FileNotFoundError:
        pass


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", required=True)
    parser.add_argument("--instance-dir", required=True)
    parser.add_argument("--ingest-url", required=True)
    parser.add_argument("--video-encoder", required=True)
    parser.add_argument("--supervisor-pid-file")
    args = parser.parse_args()

    instance_dir = pathlib.Path(args.instance_dir)
    instance_dir.mkdir(parents=True, exist_ok=True)
    fifo_path = instance_dir / "stream.mkv"
    ffmpeg_pid_path = instance_dir / "ffmpeg.pid"
    scrcpy_pid_path = instance_dir / "scrcpy.pid"
    supervisor_pid_path = pathlib.Path(args.supervisor_pid_file) if args.supervisor_pid_file else None

    remove_if_exists(fifo_path)
    os.mkfifo(fifo_path)
    if supervisor_pid_path is not None:
        write_pid(supervisor_pid_path, os.getpid())

    children = []

    def terminate_children(*_args):
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
        time.sleep(0.8)
        for child in reversed(children):
            if child.poll() is None:
                child.kill()
        if supervisor_pid_path is not None:
            remove_if_exists(supervisor_pid_path)
        remove_if_exists(ffmpeg_pid_path)
        remove_if_exists(scrcpy_pid_path)
        remove_if_exists(fifo_path)
        sys.exit(0)

    signal.signal(signal.SIGTERM, terminate_children)
    signal.signal(signal.SIGINT, terminate_children)

    ffmpeg_cmd = [
        "ffmpeg",
        "-hide_banner",
        "-loglevel",
        "info",
        "-reconnect",
        "1",
        "-reconnect_streamed",
        "1",
        "-reconnect_delay_max",
        "5",
        "-i",
        str(fifo_path),
        "-map",
        "0:v:0",
        "-c",
        "copy",
        "-f",
        "flv",
        args.ingest_url,
    ]
    scrcpy_cmd = [
        "scrcpy",
        "-s",
        args.serial,
        "--no-window",
        "--no-audio",
        "--video-codec=h264",
        f"--video-encoder={args.video_encoder}",
        "--max-fps=30",
        "--video-bit-rate=4M",
        "--display-buffer=0",
        f"--record={fifo_path}",
        "--record-format=mkv",
    ]

    print(f"[bridge] serial={args.serial}", flush=True)
    print(f"[bridge] fifo={fifo_path}", flush=True)
    print(f"[bridge] ingest={args.ingest_url}", flush=True)

    MAX_RESTARTS = 3
    restart_count = 0

    def start_ffmpeg():
        proc = subprocess.Popen(ffmpeg_cmd)
        write_pid(ffmpeg_pid_path, proc.pid)
        return proc

    def start_scrcpy():
        proc = subprocess.Popen(scrcpy_cmd)
        write_pid(scrcpy_pid_path, proc.pid)
        return proc

    ffmpeg = start_ffmpeg()
    children.append(ffmpeg)
    time.sleep(0.8)

    scrcpy = start_scrcpy()
    children.append(scrcpy)

    while True:
        ffmpeg_dead = ffmpeg.poll() is not None
        scrcpy_dead = scrcpy.poll() is not None

        if ffmpeg_dead or scrcpy_dead:
            which = "ffmpeg" if ffmpeg_dead else "scrcpy"
            code = ffmpeg.returncode if ffmpeg_dead else scrcpy.returncode
            print(f"[bridge] {which} exited with {code}", flush=True)

            restart_count += 1
            if restart_count > MAX_RESTARTS:
                print(f"[bridge] max restarts ({MAX_RESTARTS}) reached, giving up", flush=True)
                terminate_children()

            backoff = min(2 ** restart_count, 10)
            print(f"[bridge] restart #{restart_count}, backoff {backoff}s", flush=True)

            # kill the surviving process before restarting both
            for child in children:
                if child.poll() is None:
                    child.terminate()
            time.sleep(0.5)
            for child in children:
                if child.poll() is None:
                    child.kill()
            children.clear()

            # recreate FIFO
            remove_if_exists(fifo_path)
            os.mkfifo(fifo_path)

            time.sleep(backoff)

            ffmpeg = start_ffmpeg()
            children.append(ffmpeg)
            time.sleep(0.8)
            scrcpy = start_scrcpy()
            children.append(scrcpy)
            print(f"[bridge] restarted both processes", flush=True)

        time.sleep(1.0)


if __name__ == "__main__":
    raise SystemExit(main())
