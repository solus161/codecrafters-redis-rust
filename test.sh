#!/bin/bash
{
  # ECHO grape
  printf "*2\r\n\$4\r\nECHO\r\n\$6\r\nbanana\r\n"
  
  # PING
  printf "*1\r\n\$4\r\nPI"  # partial
  sleep 0.2
  printf "NG\r\n"           # remaining

  sleep 0.2
  printf "*"
  sleep 0.2
  printf "1"
  sleep 0.2
  printf "\r"
  sleep 0.2
  printf "\n"
  sleep 0.2
  printf "\$"
  sleep 0.2
  printf "4"
  sleep 0.2
  printf "\r"
  sleep 0.2
  printf "\n"
  sleep 0.2
  printf "P"
  sleep 0.2
  printf "I"
  sleep 0.2
  printf "N"
  sleep 0.2
  printf "G"
  sleep 0.2
  printf "\r"
  sleep 0.2
  printf "\n"

  # ECHO "Hello There! General Kenobiii!"
  printf "*2\r\n"
  sleep 0.2
  printf "\$4\r"
  sleep 0.2
  printf "\n"
  sleep 0.2
  printf "E"
  printf "CHO\r\n"
  sleep 0.2
  printf "\$32\r\n"
  sleep 0.2
  printf '"Hello There! '
  sleep 0.2
  printf 'General Kenobiii!"'
  printf "\r\n"

  # SET foo bar
  printf "*3\r\n\$3\r\nSET\r\n\$3\r\nfoo\r\n\$3\r\nbar\r\n"

  # GET foo
  printf "*2\r\n\$3\r\nGET\r\n\$3\r\nfoo\r\n"

  # GET bar
  printf "*2\r\n\$3\r\nGET\r\n\$3\r\nbar\r\n"

} | nc localhost 6379
