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
} | nc localhost 6379
