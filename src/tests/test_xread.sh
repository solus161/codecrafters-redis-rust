#!/bin/bash
{
  printf "*5\r\n\$4\r\nXADD\r\n\$5\r\ngrape\r\n\$3\r\n0-1\r\n\$11\r\ntemperature\r\n\$2\r\n56\r\n"
  printf "*4\r\n\$5\r\nXREAD\r\n\$7\r\nstreams\r\n\$5\r\ngrape\r\n\$3\r\n0-0\r\n"

} | nc localhost 6379
