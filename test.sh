#!/bin/bash
{
  printf "*1\r\n\$4\r\nPI"  # partial
  sleep 1.0
  printf "NG\r\n"           # remaining

  sleep 1.0
  printf "*"
  sleep 1.0
  printf "1"
  sleep 1.0
  printf "\r"
  sleep 1.0
  printf "\n"
  sleep 1.0
  printf "\$"
  sleep 1.0
  printf "4"
  sleep 1.0
  printf "\r"
  sleep 1.0
  printf "\n"
  sleep 1.0
  printf "P"
  sleep 1.0
  printf "I"
  sleep 1.0
  printf "N"
  sleep 1.0
  printf "G"
  sleep 1.0
  printf "\r"
  sleep 1.0
  printf "\n"

  # ECHO "Hello There! General Kenobiii!"
  printf "*2\r\n"
  sleep 1.0
  printf "\$4\r"
  sleep 1.0
  printf "\n"
  sleep 1.0
  printf "E"
  printf "CHO\r\n"
  sleep 1.0
  printf "\$32\r\n"
  sleep 1.0
  printf '"Hello There! '
  sleep 1.0
  printf 'General Kenobiii!"'
  printf "\r\n"

} | nc localhost 6379
