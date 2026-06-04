#!/bin/bash
{
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.0\r\n\$4\r\nhehe\r\n"

} | nc localhost 6379
