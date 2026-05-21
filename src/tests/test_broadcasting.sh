#!/bin/bash
{
  printf "*1\r\n\$4\r\nPING\r\n"
  sleep 0.1
  printf "*3\r\n\$8\r\nREPLCONF\r\n\$14\r\nlistening-port\r\n\$4\r\n6380\r\n"
  sleep 0.1
  printf "*3\r\n\$8\r\nREPLCONF\r\n\$4\r\ncapa\r\n\$6\r\npsync2\r\n"
  sleep 0.1
  printf "*3\r\n\$5\r\nPSYNC\r\n\$1\r\n?\r\n\$2\r\n-1\r\n"
  sleep 1.0
  printf "*3\r\n\$8\r\nREPLCONF\r\n\$6\r\nGETACK\r\n\$1\r\n*\r\n"

} | nc localhost 6379
