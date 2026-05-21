#!/bin/bash
{
  printf "*2\r\n\$4\r\ninfo\r\n\$11\r\nreplication\r\n"
  printf "*3\r\n\$8\r\nREPLCONF\r\n\$6\r\nGETACK\r\n\$1\r\n*\r\n"

} | nc localhost 6379
